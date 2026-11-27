"""The files of the theme copy a landing brings: new or changed against a games checkout's theme folder.

    python3 runway/landlist.py COPY_THEME_DIR TARGET_THEME_DIR

Prints one path per line (under theme/) for each file the copy's art.txt or look.txt names, or
that is art.txt, look.txt or made-from.txt, and that the target lacks or holds differently.
Files nothing names (kept alternates, backups) are listed on stderr and left out. Files the
target holds that the copy's rules no longer name are listed on stderr too.
"""
import filecmp
import os
import sys


def main():
    copy, target = sys.argv[1], sys.argv[2]
    rules = open(f'{copy}/art.txt').read() + open(f'{copy}/look.txt').read()
    named = set(w for w in rules.split() if '/' in w and '.' in w.split('/')[-1])
    keep = {'art.txt', 'look.txt', 'made-from.txt'}
    out, loose = [], []
    for root, _, files in os.walk(copy):
        for f in sorted(files):
            rel = os.path.relpath(os.path.join(root, f), copy)
            if f.startswith('.'):
                continue
            t = os.path.join(target, rel)
            if os.path.exists(t) and filecmp.cmp(os.path.join(root, f), t, shallow=False):
                continue
            if rel in keep or rel in named:
                out.append(rel)
            else:
                loose.append(rel)
    gone = []
    for root, _, files in os.walk(target):
        for f in sorted(files):
            rel = os.path.relpath(os.path.join(root, f), target)
            if rel not in keep and rel not in named and not f.startswith('.'):
                gone.append(rel)
    missing = sorted(n for n in named if not os.path.exists(os.path.join(copy, n)))
    for rel in sorted(out):
        print(rel)
    print(len(out), 'to bring;', len(loose), 'named by nothing, left out:', ' '.join(sorted(loose)), file=sys.stderr)
    print(len(gone), 'in the target no rule names:', ' '.join(sorted(gone)), file=sys.stderr)
    print(len(missing), 'named but not in the copy:', ' '.join(missing), file=sys.stderr)


if __name__ == '__main__':
    main()
