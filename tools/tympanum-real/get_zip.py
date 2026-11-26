#!/usr/bin/env python3
"""Unpack the members of a remote zip into a folder by HTTP range requests (round 42,
tympanum-real): the drums library's DrumGizmo kits, never the whole zip on disk.

    python -I tools/tympanum-real/get_zip.py <url> <dest dir> [--strip PREFIX] [--match GLOB ...]
                                             [--max-gb N] [--list]

It reads with `tools/sound-refs/fetch.py`'s `RangeFile` (the proxy's HTML pages waited out,
never a bypass), CRC-checks each member through `zipfile`, writes `<name>.part` and renames it,
skips members already there at their size, and refuses to start when F: (the destination's
drive) would fall below 20 GB free. Afterwards `fetch.py record <set>` catalogues the files.
"""
import argparse
import fnmatch
import shutil
import sys
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "sound-refs"))
import fetch  # noqa: E402

FLOOR = 20 << 30


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("url")
    ap.add_argument("dest")
    ap.add_argument("--strip", default="")
    ap.add_argument("--match", nargs="*", default=["*"])
    ap.add_argument("--max-gb", type=float, default=8.0)
    ap.add_argument("--list", action="store_true")
    a = ap.parse_args()
    dest = Path(a.dest)
    src = fetch.open_remote(a.url)
    with src, zipfile.ZipFile(src) as zf:
        members = [i for i in zf.infolist() if not i.is_dir()
                   and any(fnmatch.fnmatchcase(i.filename, g) for g in a.match)]
        total = sum(i.file_size for i in members)
        print(f"{len(members)} members, {total / 1e9:.2f} GB", flush=True)
        if a.list:
            for i in members:
                print(i.file_size, i.filename)
            return
        if total > a.max_gb * 1e9:
            sys.exit(f"{total / 1e9:.2f} GB is over --max-gb {a.max_gb}")
        dest.mkdir(parents=True, exist_ok=True)
        free = shutil.disk_usage(dest).free
        if free - total < FLOOR:
            sys.exit(f"only {free / 2**30:.1f} GiB free: {total / 2**30:.1f} GiB would leave under 20")
        done = 0
        for k, i in enumerate(members):
            name = i.filename[len(a.strip):] if i.filename.startswith(a.strip) else i.filename
            out = dest / name
            if out.exists() and out.stat().st_size == i.file_size:
                done += i.file_size
                continue
            out.parent.mkdir(parents=True, exist_ok=True)
            tmp = out.with_name(out.name + ".part")
            for t in range(3):
                try:
                    with zf.open(i) as f, open(tmp, "wb") as o:
                        shutil.copyfileobj(f, o, 1 << 20)
                    break
                except zipfile.BadZipFile as e:
                    if t == 2:
                        raise
                    print(f"  {i.filename}: {e}; again", flush=True)
            tmp.replace(out)
            done += i.file_size
            if k % 50 == 0:
                print(f"  {k + 1}/{len(members)}  {done / 1e9:.2f} GB", flush=True)
        print("done", flush=True)


if __name__ == "__main__":
    main()
