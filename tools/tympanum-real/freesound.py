#!/usr/bin/env python3
"""The drums library's Freesound part (round 42, tympanum-real): each sound's page read for
its licence, its public HQ preview (128 kb/s MP3; the originals need a login) downloaded.

    python -I tools/tympanum-real/freesound.py <dest dir> sound:<id> pack:<id> … [--max N]

Only CC0 and CC BY (3.0/4.0) sounds are kept: every sound's own page is read for its licence
link (a pack can mix licences), and any other licence is skipped and listed. Writes
`<dest>/manifest.json` (id, title, user, licence, page, preview, tags, description's first
600 characters) and `<dest>/<id>.mp3`. A preview is checked to start with ID3 or an MPEG
frame (behind the proxy an HTML page may come for ~10 minutes: the script waits and tries
again). Polite: a few seconds between pages (Freesound answers HTTP 429 to bursts).
"""
import html
import json
import re
import subprocess
import sys
import time
from pathlib import Path

OK = {"publicdomain/zero/1.0": "CC0-1.0", "licenses/by/4.0": "CC-BY-4.0",
      "licenses/by/3.0": "CC-BY-3.0"}
MP3 = (b"ID3", b"\xff\xfb", b"\xff\xf3", b"\xff\xfa", b"\xff\xf2")


def get(url, binary=False, tries=12):
    for i in range(tries):
        r = subprocess.run(["curl", "-sSL", "--fail", "-A", "cantus-sound-refs/1.0", url],
                           capture_output=True)
        if r.returncode == 0:
            body = r.stdout
            if binary and not body.startswith(MP3):
                print(f"  not an MP3 yet ({body[:40]!r}); waiting", flush=True)
            else:
                return body if binary else body.decode("utf-8", "replace")
        else:
            print(f"  {url}: curl {r.returncode} {r.stderr[:120]!r}; waiting", flush=True)
        time.sleep(30 if i < 3 else 60)
    raise SystemExit(f"{url}: gave up")


def pack_sounds(pid):
    ids, page = [], 1
    while True:
        t = get(f"https://freesound.org/people/x/packs/{pid}/?page={page}")
        found = re.findall(r'/people/[^/"]+/sounds/(\d+)/', t)
        new = [i for i in dict.fromkeys(found) if i not in ids]
        if not new:
            break
        ids += new
        if f"?page={page + 1}" not in t:
            break
        page += 1
        time.sleep(3)
    return ids


def sound(sid):
    t = get(f"https://freesound.org/s/{sid}/")
    lic = None
    for path, spdx in OK.items():
        if f"creativecommons.org/{path}" in t:
            lic = spdx
    other = re.findall(r"creativecommons.org/licenses/([a-z\-]+/[0-9.]+)", t)
    prev = re.findall(r"https://cdn\.freesound\.org/previews/[^\"']*-hq\.mp3", t)
    title = re.search(r"<title>(.*?)</title>", t, re.S)
    user = re.search(r"/people/([^/\"]+)/sounds/" + sid, t)
    desc = re.search(r'<div[^>]*id="soundDescriptionSection"[^>]*>(.*?)</div>', t, re.S) or \
        re.search(r'name="description" content="([^"]*)"', t)
    tags = re.findall(r'/browse/tags/([^/"]+)/', t)
    return {
        "id": int(sid),
        "title": html.unescape(title.group(1).strip()) if title else "",
        "user": user.group(1) if user else "",
        "licence": lic,
        "licence_seen": sorted(set(other)),
        "page": f"https://freesound.org/s/{sid}/",
        "preview": prev[0] if prev else None,
        "tags": sorted(set(tags))[:20],
        "description": html.unescape(re.sub(r"<[^>]+>", " ", desc.group(1)))[:600].strip() if desc else "",
    }


def main():
    dest = Path(sys.argv[1])
    dest.mkdir(parents=True, exist_ok=True)
    mpath = dest / "manifest.json"
    man = json.loads(mpath.read_text(encoding="utf-8")) if mpath.exists() else {"sounds": [], "skipped": []}
    have = {s["id"] for s in man["sounds"]} | {s["id"] for s in man["skipped"]}
    mx = 10 ** 9
    if "--max" in sys.argv:
        mx = int(sys.argv[sys.argv.index("--max") + 1])
    for item in [a for a in sys.argv[2:] if ":" in a]:
        kind, ident = item.split(":", 1)
        ids = pack_sounds(ident) if kind == "pack" else [ident]
        print(f"{item}: {len(ids)} sounds", flush=True)
        for sid in ids[:mx]:
            if int(sid) in have:
                continue
            time.sleep(3)
            s = sound(sid)
            s["from"] = item
            if not s["licence"] or not s["preview"] or (s["licence_seen"] and any(
                    x.split("/")[0] != "by" for x in s["licence_seen"])):
                man["skipped"].append(s)
                print(f"  skip {sid} {s['title'][:50]!r} licence {s['licence']} {s['licence_seen']}", flush=True)
            else:
                out = dest / f"{sid}.mp3"
                if not out.exists():
                    out.write_bytes(get(s["preview"], binary=True))
                man["sounds"].append(s)
                print(f"  {sid} {s['licence']} {s['title'][:60]!r}", flush=True)
            have.add(int(sid))
            mpath.write_text(json.dumps(man, indent=1, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__":
    main()
