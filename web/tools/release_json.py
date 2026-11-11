"""The latest release's facts for the download page, fetched at BUILD time (round 39).

    python web/tools/release_json.py web/assets/release.json

Asks GitHub's API once (with GITHUB_TOKEN when it is set, as in the site workflow) and writes
what `src/pages/download.rs` and `src/shelf.rs` read from the site itself: the version, the
date, and each file's name, size, SHA-256 and link. The visitor's browser never calls
api.github.com. When GitHub cannot be reached the file already in the repository is kept, so a
build never fails on it and never publishes an empty one.
"""

import json
import os
import sys
import urllib.request

API = "https://api.github.com/repos/luis314159/cantus-daw/releases/latest"


def fetch():
    req = urllib.request.Request(API, headers={"Accept": "application/vnd.github+json",
                                               "User-Agent": "cantusdaw.com-build"})
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def facts(rel):
    files = []
    for a in rel.get("assets", []):
        digest = a.get("digest") or ""
        files.append({
            "name": a["name"],
            "size": a["size"],
            "sha256": digest[7:] if digest.startswith("sha256:") else "",
            "url": a["browser_download_url"],
        })
    return {
        "version": rel["tag_name"].lstrip("v"),
        "date": rel["published_at"][:10],
        "notes": rel["html_url"],
        "files": files,
    }


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "assets/release.json"
    try:
        data = facts(fetch())
    except Exception as e:  # noqa: BLE001 — any failure keeps the committed file
        print(f"release_json: GitHub not reached ({e}); keeping {out}", file=sys.stderr)
        return
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        json.dump(data, f, indent=1, ensure_ascii=False)
        f.write("\n")
    print(f"release_json: {data['version']} ({data['date']}), {len(data['files'])} files -> {out}")


if __name__ == "__main__":
    main()
