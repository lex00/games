"""Tests for rom_text.py. Every ROM here is made up: seeded random bytes in an
iNES header, registered in a throwaway roms/nes.toml. No game data."""

import contextlib
import hashlib
import io
import os
import random
import subprocess
import tempfile
import unittest
from pathlib import Path

import rom_text

# A made-up program: 32KB of seeded random bytes, with a few regular runs
# (a fill, a ramp, a repeated palette, a triangle) that must never count.
PRG = bytearray(random.Random(1).randbytes(32768))
PRG[0x100:0x120] = bytes(32)
PRG[0x200:0x220] = bytes(range(32))
PRG[0x300:0x320] = bytes([0x21, 0x1A, 0x17, 0x37]) * 8
PRG[0x400:0x420] = bytes(list(range(15, -1, -1)) + list(range(16)))
PRG = bytes(PRG)
CHR = random.Random(2).randbytes(8192)
HEADER = b"NES\x1a\x02\x01\x01\x00" + bytes(8)
ROM = HEADER + PRG + CHR
SHA = hashlib.sha256(PRG + CHR).hexdigest()
# Where the tests take their bytes from: ordinary, varied data.
AT = 0x1234


def piece(n, at=AT):
    return PRG[at:at + n]


class Fixture:
    """A repo root with a registry, a ROM folder and a cache, all temporary."""

    def __init__(self, register=True, rom=ROM):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.root = self.dir / "repo"
        (self.root / "roms").mkdir(parents=True)
        (self.root / ".githooks").mkdir()
        sha = SHA if register else "0" * 64
        (self.root / "roms" / "nes.toml").write_text(
            f'[[source]]\nid = "madeup-ntsc"\nsha256 = "{sha}"\n'
        )
        self.roms = self.dir / "roms" / "nes"
        self.roms.mkdir(parents=True)
        (self.roms / "made up.nes").write_bytes(rom)
        self.cache = self.dir / "cache"

    def run(self, *args):
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code = rom_text.main([
                "--root", str(self.root), "--rom-dir", str(self.dir / "roms"),
                "--cache", str(self.cache), *args,
            ])
        return code, err.getvalue()

    def check(self, text, name="t.rs"):
        path = self.root / name
        path.write_text(text)
        return self.run("files", str(path))

    def close(self):
        self.tmp.cleanup()


class Formats(unittest.TestCase):
    def setUp(self):
        self.f = Fixture()

    def tearDown(self):
        self.f.close()

    def written(self, fmt, sep, n):
        return sep.join(fmt.format(b) for b in piece(n))

    def assert_refused(self, text):
        code, err = self.f.check(text)
        self.assertEqual(code, 1, text)
        self.assertIn("madeup-ntsc", err)
        self.assertIn(f"0x{AT:X}", err)

    def test_each_format_of_16_bytes_is_refused_and_15_is_not(self):
        for fmt, sep in [
            ("0x{:02X}", ", "), ("0x{:02x}u8", ", "), ("${:02X}", ","), ("{:02x}", " "),
            ("{:02X}", ","), ("{:d}", ", "), ("0{:02X}h", ", "), ("\\x{:02x}", ""),
            ("{:02x}", ""), ("0b{:08b}", ", "), ("%{:08b}", ","),
        ]:
            with self.subTest(fmt=fmt, sep=sep):
                self.assert_refused("const T: [u8; 16] = [" + self.written(fmt, sep, 16) + "];\n")
                code, _ = self.f.check("x = [" + self.written(fmt, sep, 15) + "]\n")
                self.assertEqual(code, 0)

    def test_a_table_over_lines_with_a_comment_per_row(self):
        rows = [", ".join(f"0x{b:02X}" for b in piece(8, AT + i)) for i in (0, 8, 16)]
        text = "const T: [u8; 24] = [\n" + "".join(
            f"    {r}, // row {i}\n" for i, r in enumerate(rows)
        ) + "];\n"
        self.assert_refused(text)

    def test_nested_arrays_and_assembler(self):
        b = piece(16)
        self.assert_refused("[" + ", ".join(f"[0x{x:02X}, 0x{y:02X}]" for x, y in zip(b[::2], b[1::2])) + "]")
        self.assert_refused("\n".join(
            "  .byte " + ",".join(f"${x:02X}" for x in b[i:i + 4]) for i in range(0, 16, 4)
        ))

    def test_quoted_lists_and_markdown_tables(self):
        b = piece(16)
        self.assert_refused("[" + ", ".join(f'"0x{x:02x}"' for x in b) + "]")
        self.assert_refused("| " + " | ".join(f"${x:02X}" for x in b) + " |")
        self.assert_refused("  hex " + b.hex())

    def test_the_message_names_file_line_and_the_fix(self):
        code, err = self.f.check("a\nb\n" + self.written("0x{:02X}", ", ", 20) + "\n")
        self.assertEqual(code, 1)
        self.assertIn("t.rs:3 (20 bytes of madeup-ntsc", err)
        self.assertIn("asset map", err)

    def test_regular_runs_in_the_rom_are_skipped(self):
        for at in (0x100, 0x200, 0x300, 0x400):
            with self.subTest(at=hex(at)):
                code, _ = self.f.check(", ".join(f"0x{b:02X}" for b in piece(32, at)))
                self.assertEqual(code, 0)

    def test_bytes_not_in_the_rom_and_prose(self):
        other = random.Random(3).randbytes(64)
        code, _ = self.f.check(other.hex() + "\n" + ", ".join(map(str, other)))
        self.assertEqual(code, 0)
        code, _ = self.f.check("a bad cafe added a dead bee to a faded bed\n")
        self.assertEqual(code, 0)


class Roms(unittest.TestCase):
    def test_no_registered_rom_means_no_check(self):
        f = Fixture(register=False)
        try:
            code, err = f.check(piece(32).hex())
            self.assertEqual((code, err), (0, ""))
            self.assertFalse(f.cache.exists())
        finally:
            f.close()

    def test_no_rom_folder_means_no_check(self):
        f = Fixture()
        try:
            (f.root / "t.rs").write_text(piece(32).hex())
            err = io.StringIO()
            with contextlib.redirect_stderr(err):
                code = rom_text.main(["--root", str(f.root), "--rom-dir", str(f.dir / "none"),
                                      "--cache", str(f.cache), "files", str(f.root / "t.rs")])
            self.assertEqual(code, 0)
        finally:
            f.close()

    def test_a_dump_with_junk_is_found_by_its_headerless_hash(self):
        junk = bytearray(HEADER)
        junk[6] |= 0x04
        junk[7:16] = b"DiskDude!"
        f = Fixture(rom=bytes(junk) + bytes(512) + PRG + CHR + b"trailing")
        try:
            code, _ = f.check(piece(16).hex())
            self.assertEqual(code, 1)
        finally:
            f.close()

    def test_the_index_is_cached_by_rom_hash_and_reused(self):
        f = Fixture()
        try:
            f.check("x")
            index = f.cache / f"{SHA}.v{rom_text.INDEX_VERSION}.keys"
            self.assertTrue(index.is_file())
            before = index.stat().st_mtime_ns
            os.utime(index, ns=(before - 10**9, before - 10**9))
            code, _ = f.check(piece(16).hex())
            self.assertEqual(code, 1)
            self.assertEqual(index.stat().st_mtime_ns, before - 10**9)
            index.write_bytes(b"")  # A damaged index is rebuilt, never trusted.
            code, _ = f.check(piece(16).hex())
            self.assertEqual(code, 1)
            self.assertGreater(index.stat().st_size, 0)
        finally:
            f.close()


class Exceptions(unittest.TestCase):
    def setUp(self):
        self.f = Fixture()

    def tearDown(self):
        self.f.close()

    def allow(self, path, data):
        (self.f.root / ".githooks" / "rom-excerpt-allowlist").write_text(
            f"# a reviewed excerpt\n{path} {hashlib.sha256(data).hexdigest()}\n"
        )

    def test_a_listed_excerpt_is_allowed_only_in_its_file(self):
        self.allow("t.rs", piece(20))
        self.assertEqual(self.f.check(piece(20).hex())[0], 0)
        self.assertEqual(self.f.check(piece(20).hex(), name="u.rs")[0], 1)

    def test_a_comment_inside_a_run_cannot_shorten_it_past_the_cap(self):
        # Read as written, the comment ends the run at 20 bytes; with it
        # blanked the run is 36, and that is the span checked against the list.
        self.allow("t.rs", piece(20))
        text = ", ".join(f"0x{b:02X}" for b in piece(20)) + ", // note\n" + ", ".join(
            f"0x{b:02X}" for b in piece(16, AT + 20)
        )
        code, err = self.f.check(text)
        self.assertEqual(code, 1)
        self.assertIn("36 bytes", err)

    def test_an_exception_covers_at_most_32_bytes(self):
        self.allow("t.rs", piece(33))
        self.assertEqual(self.f.check(piece(33).hex())[0], 1)


class Staged(unittest.TestCase):
    def setUp(self):
        self.f = Fixture()
        self.git("init", "-q")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "test")

    def tearDown(self):
        self.f.close()

    def git(self, *args):
        subprocess.run(["git", *args], cwd=self.f.root, check=True, capture_output=True)

    def stage(self, name, text):
        (self.f.root / name).write_text(text)
        self.git("add", name)

    def test_only_added_lines_count(self):
        table = ", ".join(f"0x{b:02X}" for b in piece(16))
        self.stage("old.rs", "one\n" + table + "\n")
        code, err = self.f.run("staged")
        self.assertEqual(code, 1)
        self.assertIn("old.rs:2 ", err)
        self.git("commit", "-q", "--no-verify", "-m", "fixture")  # a throwaway repo, made-up bytes
        self.stage("old.rs", "two\n" + table + "\n")
        self.assertEqual(self.f.run("staged")[0], 0)

    def test_a_path_with_a_space_and_unicode(self):
        self.stage("with space \u00e9.rs", piece(16).hex() + "\n")
        code, err = self.f.run("staged")
        self.assertEqual(code, 1)
        self.assertIn("with space \u00e9.rs:1 ", err)

    def test_line_numbers_in_a_later_hunk_and_odd_lines(self):
        self.stage("a.md", "".join(f"line {i}\n" for i in range(1, 30)))
        self.git("commit", "-q", "--no-verify", "-m", "fixture")
        lines = (self.f.root / "a.md").read_text().splitlines()
        lines[20] = "++ " + piece(16).hex()
        self.stage("a.md", "\n".join(lines) + "\n")
        code, err = self.f.run("staged")
        self.assertEqual(code, 1)
        self.assertIn("a.md:21 ", err)


if __name__ == "__main__":
    unittest.main()
