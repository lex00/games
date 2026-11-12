#!/usr/bin/env python3
"""Refuse ROM data in a commit: typed out as numbers, or a guarded file whole.

The pre-commit hook catches game images and binary dumps by their contents.
This catches the other ways ROM data gets in:

- by hash, always: a staged file (or a member of a staged zip) whose SHA-1
  is a guarded arcade chip's, or whose SHA-256 (whole, or with an iNES
  header taken off) is a guarded NES image's, under any name and even on
  the binary allowlist. This needs no ROM on the machine.
- typed out: a table typed out as numbers in source, a hex dump in a doc, a
  byte string in a test. It reads the staged diff's added lines, finds runs
  of byte values written as numbers, and refuses any run that holds 16 or
  more consecutive bytes of a guarded ROM or arcade chip present in
  $GP_ROM_DIR. With none present this part can't run, and the hook says so.

  rom_text.py staged
      Checks the staged files' hashes and the added lines of the staged
      diff (the pre-commit hook runs this), and says on stderr what it
      could only guard by hash. Exits 0 when nothing matches.
  rom_text.py files PATH...
      Checks whole files, their hashes and every line, to audit a tree.

Byte values are read as:
  0x12, $12, 0A6h, \\x12         hex with a prefix or suffix
  0b0001_0010, %00010010         binary, up to 8 digits
  12 34 ab, 12,34                bare two-digit hex (or one digit 0-9), with separators
  1234abcd...                    a bare hex string of even length (4 digits or more)
  18, 52, 171                    decimal 0 to 255, with separators

Bare tokens are tried both ways, once as hex and once as decimal. Tokens in
one run are separated only by whitespace, the characters ,;:|-"' and
brackets, and assembler byte directives (.byte, .byt, .db, db, hex);
anything else ends the run. Each
run is checked twice, as written and with line comments (//, #, ;) and
/* */ comments blanked, so a table with a comment per row is one run.

A 16-byte window too regular to say where it came from is skipped: one with
fewer than 4 distinct values, a pattern of 4 bytes or fewer repeated (a
palette set four times), or a ramp whose steps are all one size (0 1 2 3,
or the APU's triangle 3 2 1 0 0 1 2 3). These are in every ROM and in
plenty of our own code.

What is guarded is read from the repo, hashes only:
  .githooks/guarded-roms.toml   [[source]] NES images by headerless SHA-256,
                                [[chip]] arcade chips by SHA-1, with their set,
                                region and offset (gp-arcade's set table)
  roms/*.toml, */roms.toml      [[source]] NES images
  package.toml, */package.toml  [[rom.entry]]: a NES entry's sha256; an arcade
                                entry's `system` names a set whose chips are
                                guarded (they must be in guarded-roms.toml)

The ROMs are found by hash, as `gp` finds them: every file in $GP_ROM_DIR
with an iNES header whose headerless SHA-256 is guarded, and every guarded
chip in a zip (a MAME romset) or loose file there, by its SHA-1. A set's
chips that load one after another in a region are also read joined, so a
table typed across two chips is found. The 16-byte windows are indexed once
into $GP_HOME/work/rom-guard/ (sorted 8-byte prefixes, in a file named by the
ROM's hash, or by a hash of the hashes for several) and reused on every
later commit until the set of ROMs changes.

A reviewed exception for a short excerpt (AGENTS.md allows "a few bytes" to
document how revisions differ) goes in .githooks/rom-excerpt-allowlist as
`<path> <sha256 of the matched bytes>`, which the refusal message prints.
An exception covers at most MAX_EXCEPTION bytes. A whole guarded file has
no exception.

Exits 0 when nothing matches, 1 when something does, 2 when it can't run.
"""

import argparse
import array
import bisect
import codecs
import hashlib
import io
import os
import re
import subprocess
import sys
from pathlib import Path

WINDOW = 16
MAX_EXCEPTION = 32
INDEX_VERSION = 1
MAX_REPORTS_PER_FILE = 5
HEADER_LEN = 16
TRAINER_LEN = 512

# One token: a byte written with a prefix or suffix, or a bare hex or decimal
# number. The lookarounds keep it from starting or ending inside a word.
TOKEN = re.compile(
    r"\\x(?P<esc>[0-9A-Fa-f]{2})"
    r"|(?<![\w$%])(?:0[xX](?P<px>[0-9A-Fa-f]+)|\$(?P<dollar>[0-9A-Fa-f]{1,2})"
    r"|0[bB](?P<bin>[01][01_]*)|%(?P<pct>[01]{1,8})"
    r"|(?P<hsuf>[0-9][0-9A-Fa-f]{0,2})[hH]|(?P<bare>[0-9A-Fa-f]+))"
    r"(?:_?[ui]8)?(?![\w$])"
)
# What may sit between two tokens of one run.
GAP = re.compile(r"(?:[\s,;:|\-\"'\[\](){}]|(?i:\.byte|\.byt|\.db|\bdb|\bhex)\b)*")
LINE_COMMENT = re.compile(r"(?://|#|;)[^\n]*")
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)


class CantRun(Exception):
    pass


# ---------------------------------------------------------------- the ROMs


def identify_nes(data):
    """The PRG and CHR data of an iNES image, header and trainer removed, by
    the rules in gp_evidence::source::identify_nes. None if it isn't one."""
    if len(data) < HEADER_LEN or data[:4] != b"NES\x1a":
        return None
    h = data[:HEADER_LEN]

    def nes20_size(lsb, msb, unit):
        if msb == 0x0F:
            return (1 << (lsb >> 2)) * ((lsb & 3) * 2 + 1)
        return (msb << 8 | lsb) * unit

    sizes20 = (nes20_size(h[4], h[9] & 0x0F, 16384), nes20_size(h[5], h[9] >> 4, 8192))
    if h[7] & 0x0C == 0x08 and HEADER_LEN + sizes20[0] + sizes20[1] <= len(data):
        prg, chr_ = sizes20
    else:
        prg, chr_ = h[4] * 16384, h[5] * 8192
    declared = prg + chr_
    trainer = TRAINER_LEN if h[6] & 0x04 and len(data) - HEADER_LEN >= declared + TRAINER_LEN else 0
    start = HEADER_LEN + trainer
    if len(data) - start < declared:
        return None
    return data[start:start + declared]


class Registry:
    """What the repo guards: NES images by headerless SHA-256 ({sha256: id})
    and arcade chips by SHA-1 ({sha1: [chip row, ...]}, a row a set that
    has the chip), plus notes on what it names and can't guard."""

    def __init__(self):
        self.nes = {}
        self.chips = {}
        self.notes = []

    def __bool__(self):
        return bool(self.nes or self.chips)


def chip_label(rows):
    """A chip for people: its file name and every set that has it."""
    sets = sorted({r["set"] for r in rows})
    return f"arcade chip {rows[0]['name']} ({', '.join(sets)})"


def load_registry(root):
    """The Registry from .githooks/guarded-roms.toml, roms/*.toml,
    */roms.toml and every package.toml's [[rom.entry]] under `root`."""
    import tomllib

    reg = Registry()
    all_chips = []

    def read(path):
        with open(path, "rb") as f:
            return tomllib.load(f)

    lists = [root / ".githooks" / "guarded-roms.toml", *sorted((root / "roms").glob("*.toml")),
             *sorted(root.glob("*/roms.toml"))]
    for path in lists:
        if not path.is_file():
            continue
        data = read(path)
        for entry in data.get("source", []):
            if "sha256" in entry and "id" in entry:
                reg.nes[entry["sha256"].lower()] = entry["id"]
        for chip in data.get("chip", []):
            if {"set", "name", "sha1"} <= chip.keys():
                all_chips.append(chip)
    by_set = {}
    for chip in all_chips:
        by_set.setdefault(chip["set"], []).append(chip)
        reg.chips.setdefault(chip["sha1"].lower(), []).append(chip)
    for path in [root / "package.toml", *sorted(root.glob("*/package.toml"))]:
        if not path.is_file():
            continue
        for entry in read(path).get("rom", {}).get("entry", []):
            platform = entry.get("platform", "nes")
            if platform == "nes" and "sha256" in entry and "id" in entry:
                reg.nes[entry["sha256"].lower()] = entry["id"]
            elif platform == "arcade" and entry.get("system") not in by_set:
                rel = path.relative_to(root)
                reg.notes.append(
                    f"{rel} names the arcade set {entry.get('system')!r}, whose chips "
                    ".githooks/guarded-roms.toml doesn't list, so they aren't guarded"
                )
    return reg


def gp_home():
    return Path(os.environ.get("GP_HOME") or Path.home() / ".gameprinter")


def rom_dir():
    return Path(os.environ.get("GP_ROM_DIR") or gp_home() / "roms")


def zip_members(data):
    """[(name, bytes)] of a zip's files, or [] when it isn't one."""
    import zipfile

    try:
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            return [(i.filename, z.read(i)) for i in z.infolist() if not i.is_dir()]
    except (zipfile.BadZipFile, OSError, ValueError, NotImplementedError, EOFError):
        return []


def find_roms(folder, registry):
    """[(id, sha256, data)] for each guarded NES image and arcade chip in
    `folder`, and for each run of a set's chips that load one after another
    in a region (joined, so a table typed across two chips is found)."""
    if not registry or not folder.is_dir():
        return []
    if isinstance(registry, dict):  # {sha256: id}, NES only
        nes = registry
        registry = Registry()
        registry.nes = nes
    found = {}
    chips = {}  # sha1: data
    sizes = {int(r["size"]) for rows in registry.chips.values() for r in rows if "size" in r}
    for dirpath, _dirs, files in os.walk(folder, followlinks=True):
        for name in sorted(files):
            path = Path(dirpath) / name
            try:
                size = path.stat().st_size
                with open(path, "rb") as f:
                    magic = f.read(4)
                    if magic == b"NES\x1a" and registry.nes:
                        data = identify_nes(magic + f.read())
                        if data is None:
                            continue
                        sha = hashlib.sha256(data).hexdigest()
                        if sha in registry.nes and sha not in found:
                            found[sha] = (registry.nes[sha], sha, data)
                    elif magic == b"PK\x03\x04" and registry.chips:
                        for _, data in zip_members(magic + f.read()):
                            sha1 = hashlib.sha1(data).hexdigest()
                            if sha1 in registry.chips:
                                chips[sha1] = data
                    elif size in sizes:
                        data = magic + f.read()
                        sha1 = hashlib.sha1(data).hexdigest()
                        if sha1 in registry.chips:
                            chips[sha1] = data
            except OSError:
                continue
    for sha1, data in chips.items():
        sha = hashlib.sha256(data).hexdigest()
        found.setdefault(sha, (chip_label(registry.chips[sha1]), sha, data))
    # The chips of each set that load one after another in a region, joined.
    by_set = {}
    for sha1, rows in registry.chips.items():
        for r in rows:
            if sha1 in chips and "region" in r and "offset" in r:
                by_set.setdefault((r["set"], r["region"]), []).append((int(r["offset"]), sha1))
    for (set_name, region), pieces in sorted(by_set.items()):
        pieces.sort()
        run = []
        for off, sha1 in pieces + [(None, None)]:
            if run and off is not None and run[-1][0] + len(chips[run[-1][1]]) == off:
                run.append((off, sha1))
                continue
            if len(run) > 1:
                data = b"".join(chips[s] for _, s in run)
                sha = hashlib.sha256(data).hexdigest()
                end = run[-1][0] + len(chips[run[-1][1]])
                label = f"arcade set {set_name}, {region} 0x{run[0][0]:X}-0x{end - 1:X}"
                found.setdefault(sha, (label, sha, data))
            run = [(off, sha1)] if off is not None else []
    return sorted(found.values())


def hash_refusals(path, data, registry):
    """A message for each way `data`, a whole staged file, is a guarded ROM
    or chip by its hash: itself, with an iNES header taken off, or a member
    of a zip."""
    out = []

    def check(blob, where):
        sha1 = hashlib.sha1(blob).hexdigest()
        if sha1 in registry.chips:
            out.append(f"pre-commit: refusing {path} ({where}{chip_label(registry.chips[sha1])}, by its SHA-1)")
        sha = hashlib.sha256(blob).hexdigest()
        if sha in registry.nes:
            out.append(f"pre-commit: refusing {path} ({where}the PRG and CHR data of {registry.nes[sha]}, by its SHA-256)")
        image = identify_nes(blob)
        if image is not None:
            sha = hashlib.sha256(image).hexdigest()
            if sha in registry.nes:
                out.append(f"pre-commit: refusing {path} ({where}an image of {registry.nes[sha]}, by its headerless SHA-256)")

    check(data, "")
    if data[:4] == b"PK\x03\x04":
        for name, member in zip_members(data):
            check(member, f"{name} in it is ")
    return out


def staged_blobs(root):
    """(path, bytes) of each staged file the commit adds or changes."""
    names = subprocess.run(
        ["git", "diff", "--cached", "--name-only", "--no-renames", "--diff-filter=ACMRT", "-z"],
        cwd=root, capture_output=True, check=True,
    ).stdout.split(b"\0")
    names = [n for n in names if n and b"\n" not in n]
    if not names:
        return
    p = subprocess.run(
        ["git", "cat-file", "--batch"], cwd=root, check=True, capture_output=True,
        input=b"".join(b":" + n + b"\n" for n in names),
    ).stdout
    at = 0
    for n in names:
        end = p.index(b"\n", at)
        header = p[at:end].split()
        at = end + 1
        if len(header) < 3 or header[1] == b"missing":
            continue
        size = int(header[2])
        yield n.decode("utf-8", "surrogateescape"), p[at:at + size]
        at += size + 1


class Rom:
    def __init__(self, source_id, sha256, data):
        self.id, self.sha256, self.data = source_id, sha256, data


class Index:
    """The sorted 8-byte prefixes of every 16-byte window of every ROM,
    cached under `cache` by the ROMs' hashes, so a normal commit only reads
    one file."""

    def __init__(self, roms, cache):
        self.roms = roms
        key = hashlib.sha256(" ".join(sorted(r.sha256 for r in roms)).encode()).hexdigest()
        name = roms[0].sha256 if len(roms) == 1 else f"set-{key}"
        path = cache / f"{name}.v{INDEX_VERSION}.keys" if cache else None
        self.keys = self._load(path) if path and path.is_file() else None
        if self.keys is not None and not self._covers_roms():
            self.keys = None  # A damaged index; build it again.
        if self.keys is None:
            self.keys = self._build()
            if path:
                self._save(path, cache)

    @staticmethod
    def _load(path):
        raw = path.read_bytes()
        if len(raw) % 8:
            return None
        keys = array.array("Q")
        keys.frombytes(raw)
        if sys.byteorder != "little":
            keys.byteswap()
        return keys

    def _covers_roms(self):
        """Whether 64 windows spread over each ROM are in the index."""
        for rom in self.roms:
            last = len(rom.data) - WINDOW
            for i in range(64):
                at = last * i // 63
                key = int.from_bytes(rom.data[at:at + 8], "little")
                j = bisect.bisect_left(self.keys, key)
                if j == len(self.keys) or self.keys[j] != key:
                    return False
        return True

    def _build(self):
        keys = set()
        for rom in self.roms:
            data = rom.data
            keys.update(int.from_bytes(data[i:i + 8], "little") for i in range(len(data) - WINDOW + 1))
        return array.array("Q", sorted(keys))

    def _save(self, path, cache):
        try:
            cache.mkdir(parents=True, exist_ok=True)
            out = array.array("Q", self.keys)
            if sys.byteorder != "little":
                out.byteswap()
            tmp = path.with_suffix(f".tmp{os.getpid()}")
            tmp.write_bytes(out.tobytes())
            os.replace(tmp, path)
        except OSError:
            pass  # An unwritable cache only costs time.

    def find(self, window):
        """(rom, offset) of the first ROM holding `window`, or None."""
        key = int.from_bytes(window[:8], "little")
        keys = self.keys
        i = bisect.bisect_left(keys, key)
        if i == len(keys) or keys[i] != key:
            return None
        for rom in self.roms:
            at = rom.data.find(window)
            if at >= 0:
                return rom, at
        return None


# ---------------------------------------------------------- reading numbers


def trivial(window):
    """A window too regular to say where it came from: fewer than 4 distinct
    values, a pattern of 4 bytes or fewer repeated, or a ramp whose steps
    are all the same size up, down or flat (0 1 2 3, or 3 2 1 0 0 1 2 3)."""
    if len(set(window)) < 4:
        return True
    if any(window[p:] == window[:-p] for p in range(1, 5)):
        return True
    steps = {abs(((b - a + 128) & 0xFF) - 128) for a, b in zip(window, window[1:])}
    return len(steps - {0}) <= 1


def token_bytes(m, mode):
    """The bytes a token stands for in `mode` ("hex" or "dec"), or None."""
    if m["esc"] is not None:
        return bytes([int(m["esc"], 16)])
    if m["dollar"] is not None:
        return bytes([int(m["dollar"], 16)])
    for group in ("bin", "pct"):
        if m[group] is not None:
            digits = m[group].replace("_", "")
            return bytes([int(digits, 2)]) if len(digits) <= 8 else None
    if m["hsuf"] is not None:
        value = int(m["hsuf"], 16)
        return bytes([value]) if value <= 255 else None
    if m["px"] is not None:
        digits = m["px"]
        if len(digits) <= 2:
            return bytes([int(digits, 16)])
        if len(digits) >= 2 * WINDOW and len(digits) % 2 == 0:
            return bytes.fromhex(digits)
        return None
    digits = m["bare"]
    if mode == "dec":
        if len(digits) <= 3 and digits.isdigit() and int(digits) <= 255:
            return bytes([int(digits)])
        return None
    if len(digits) == 2 or (len(digits) == 1 and digits.isdigit()):
        return bytes([int(digits, 16)])
    if len(digits) >= 4 and len(digits) % 2 == 0:
        return bytes.fromhex(digits)
    return None


def runs(text):
    """(bytes, [text offset of each byte]) for each run of byte tokens."""
    matches = list(TOKEN.finditer(text))
    for mode in ("hex", "dec"):
        buf, where, last_end = bytearray(), [], None
        for m in matches:
            value = token_bytes(m, mode)
            joined = last_end is not None and GAP.fullmatch(text, last_end, m.start())
            if value is None or not joined:
                if len(buf) >= WINDOW:
                    yield bytes(buf), where
                buf, where = bytearray(), []
            if value is None:
                last_end = None
                continue
            buf += value
            where += [m.start()] * len(value)
            last_end = m.end()
        if len(buf) >= WINDOW:
            yield bytes(buf), where


def blank_comments(text):
    def blank(m):
        return re.sub(r"[^\n]", " ", m.group(0))

    return LINE_COMMENT.sub(blank, BLOCK_COMMENT.sub(blank, text))


def find_spans(text, index):
    """(text offset, rom, rom offset, matched bytes) for each ROM span of 16
    or more bytes in `text`, sorted, longest first at each offset."""
    longest, streams = {}, set()
    for variant in (text, blank_comments(text)):
        for stream, where in runs(variant):
            # Comments blanked and decimal readings often give the same run.
            if (stream, where[0]) in streams:
                continue
            streams.add((stream, where[0]))
            i = 0
            while i + WINDOW <= len(stream):
                window = stream[i:i + WINDOW]
                hit = index.find(window)
                if hit is None or trivial(window):
                    i += 1
                    continue
                rom, at = hit
                n = WINDOW
                while i + n < len(stream) and at + n < len(rom.data) and stream[i + n] == rom.data[at + n]:
                    n += 1
                # A comment can cut a run short in one reading; keep the
                # longest span at each place so the exception cap holds.
                key = (where[i], rom.id, at)
                if key not in longest or n > len(longest[key][3]):
                    longest[key] = (where[i], rom, at, stream[i:i + n])
                i += n
    return sorted(longest.values(), key=lambda s: (s[0], -len(s[3])))


# ------------------------------------------------------------ the checking


def load_exceptions(path):
    allowed = set()
    if path.is_file():
        for line in path.read_text().splitlines():
            line = line.split("#", 1)[0].strip()
            if line:
                parts = line.rsplit(None, 1)
                if len(parts) == 2:
                    allowed.add((parts[0], parts[1].lower()))
    return allowed


def check_text(path, text, first_line, index, allowed):
    """A message for each span in `text` (which starts at line `first_line`
    of `path`) that no exception allows."""
    out = []
    for offset, rom, at, span in find_spans(text, index):
        digest = hashlib.sha256(span).hexdigest()
        if len(span) <= MAX_EXCEPTION and (path, digest) in allowed:
            continue
        line = first_line + text.count("\n", 0, offset)
        # The same bytes in other guarded ROMs or chips (sets share code);
        # a set's joined chips go without saying.
        also = [r.id for r in index.roms
                if r is not rom and not r.id.startswith("arcade set ") and r.data.find(span) >= 0]
        more = f", also in {'; '.join(also)}" if also else ""
        out.append(
            f"pre-commit: refusing {path}:{line} ({len(span)} bytes of {rom.id} at ROM offset "
            f"0x{at:X}{more}, written as numbers; sha256 {digest})"
        )
    return out


def staged_hunks(root):
    """(path, first line, added text) for each hunk of the staged diff."""
    out = subprocess.run(
        ["git", "-c", "core.quotePath=false", "diff", "--cached", "-U0", "--no-color",
         "--no-ext-diff", "--no-textconv", "--no-renames", "--diff-filter=ACMRT",
         "--src-prefix=a/", "--dst-prefix=b/"],
        cwd=root, capture_output=True, check=True,
    ).stdout.decode("utf-8", "surrogateescape")
    path, first, lines, header = None, 0, [], False
    for line in out.split("\n"):
        if line.startswith("diff --git "):
            if lines:
                yield path, first, "\n".join(lines)
            path, lines, header = None, [], True
        elif header and line.startswith("+++ "):
            name = line[4:].rstrip("\t")
            if name.startswith('"') and name.endswith('"'):
                raw = name[1:-1].encode("utf-8", "surrogateescape")
                name = codecs.escape_decode(raw)[0].decode("utf-8", "surrogateescape")
            path = name[2:] if name.startswith("b/") else None
        elif line.startswith("@@ "):
            header = False
            if lines:
                yield path, first, "\n".join(lines)
            lines = []
            m = re.match(r"@@ -\S+ \+(\d+)", line)
            first = int(m.group(1)) if m else 0
        elif not header and line.startswith("+") and path is not None:
            lines.append(line[1:])
    if lines:
        yield path, first, "\n".join(lines)


def by_hash_only(registry, found, folder):
    """What the typed-out check couldn't cover, for the hook to say: the
    guarded NES images and arcade chips with none of their kind found."""
    labels = [r[0] for r in found]
    out = []
    if registry.nes and not any(not l.startswith("arcade ") for l in labels):
        out.append(f"no guarded NES ROM in {folder}: {len(registry.nes)} NES images guarded "
                   "by hash only, typed-out bytes not checked")
    if registry.chips and not any(l.startswith("arcade ") for l in labels):
        out.append(f"no arcade romset in {folder}: {len(registry.chips)} arcade chips guarded "
                   "by hash only, typed-out chip bytes not checked")
    return out


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--root", type=Path, help="repo root (default: the current git work tree)")
    p.add_argument("--rom-dir", type=Path, help="default $GP_ROM_DIR")
    p.add_argument("--cache", type=Path, help="default $GP_HOME/work/rom-guard")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("staged")
    files = sub.add_parser("files")
    files.add_argument("paths", nargs="+", type=Path)
    args = p.parse_args(argv)

    try:
        root = args.root or Path(subprocess.run(
            ["git", "rev-parse", "--show-toplevel"], capture_output=True, check=True, text=True,
        ).stdout.strip())
        try:
            registry = load_registry(root)
        except ImportError:
            raise CantRun("python3 3.11 or newer is needed (for tomllib)")
        except Exception as e:
            raise CantRun(f"reading the ROM lists: {e}")
        if not registry:
            return 0
        folder = args.rom_dir or rom_dir()
        per_file = {}
        # By hash first: a guarded file whole, with no ROM needed here.
        if args.cmd == "staged":
            blobs = staged_blobs(root)
        else:
            blobs = ((os.path.relpath(path.resolve(), root.resolve()), path.read_bytes())
                     for path in args.paths)
        for path, data in blobs:
            per_file.setdefault(path, []).extend(hash_refusals(path, data, registry))

        found = find_roms(folder, registry)
        if args.cmd == "staged":
            for line in registry.notes + by_hash_only(registry, found, folder):
                print(f"pre-commit: {line}", file=sys.stderr)
        if found:
            cache = args.cache or gp_home() / "work" / "rom-guard"
            index = Index([Rom(*r) for r in found], cache)
            allowed = load_exceptions(root / ".githooks" / "rom-excerpt-allowlist")
            if args.cmd == "staged":
                texts = staged_hunks(root)
            else:
                texts = (
                    (os.path.relpath(path.resolve(), root.resolve()), 1,
                     path.read_bytes().decode("utf-8", "surrogateescape"))
                    for path in args.paths
                )
            for path, first, text in texts:
                per_file.setdefault(path, []).extend(check_text(path, text, first, index, allowed))
    except CantRun as e:
        print(f"pre-commit: can't check staged text for ROM bytes: {e}", file=sys.stderr)
        return 2
    except (OSError, subprocess.CalledProcessError) as e:
        print(f"pre-commit: can't check staged text for ROM bytes: {e}", file=sys.stderr)
        return 2

    if not any(per_file.values()):
        return 0
    for path, messages in per_file.items():
        for line in messages[:MAX_REPORTS_PER_FILE]:
            print(line, file=sys.stderr)
        if len(messages) > MAX_REPORTS_PER_FILE:
            more = len(messages) - MAX_REPORTS_PER_FILE
            print(f"pre-commit: {path} has {more} more runs like these", file=sys.stderr)
    print(
        "ROM data never goes in the repo, even typed out as numbers in source, docs or tests.\n"
        "Lift it through the game's asset map instead (gp assets lift, docs/assets.md) and read\n"
        "the lifted files from $GP_HOME/work at build or run time; tests use made-up tables.\n"
        "A ROM image, dump or romset chip is never committed, under any name.\n"
        f"A reviewed excerpt of at most {MAX_EXCEPTION} bytes that AGENTS.md allows can be listed in\n"
        ".githooks/rom-excerpt-allowlist as `<path> <sha256>`, with a comment saying why.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:  # A crash is "couldn't run", never a pass.
        print(f"pre-commit: can't check staged text for ROM bytes: {e!r}", file=sys.stderr)
        sys.exit(2)
