#!/usr/bin/env python3
"""The site's numbers and its release feed, asked of GitHub at build time (round 39, P2-4, P3).

Writes two files into --out (default: web/assets):

  stats.json    {"stars": N, "downloads": N, "releases": N, "as_of": "YYYY-MM-DD"} — the
                public repository's stars and the download count GitHub keeps for every
                release file (installers, AppImages, disk images; the checksum and text files
                are not counted). `proof::Proof` shows a number only when it is here.
  releases.xml  an Atom feed of the releases: title, date and a link to the notes.

Nothing is invented: every number is GitHub's answer on the day of the build. When GitHub
cannot be reached, nothing is written and the script exits 0 (the site then shows no proof
and serves no stale feed); --strict makes that an error. GITHUB_TOKEN, when set, lifts the
60-requests-an-hour limit (site.yml passes the workflow's own token).

  python3 web/tools/site_data.py --out web/dist        # after `trunk build`
  python3 web/tools/site_data.py --from-json dir/      # offline: releases.json + repo.json
"""

import argparse
import datetime as dt
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path
from xml.sax.saxutils import escape

REPO = "luis314159/cantus-daw"
SITE = "https://cantusdaw.com"
API = "https://api.github.com"

# Files that are not a download of the program: checksums, signatures, notes.
NOT_A_DOWNLOAD = (".txt", ".sha256", ".sig", ".asc", ".json", ".blockmap", ".md")


def get(url, token):
    req = urllib.request.Request(url, headers={
        "Accept": "application/vnd.github+json",
        "User-Agent": "cantusdaw.com-build",
        "X-GitHub-Api-Version": "2022-11-28",
    })
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.load(resp)


def fetch(repo, token):
    """The repository and every published release, newest first."""
    info = get(f"{API}/repos/{repo}", token)
    releases, page = [], 1
    while True:
        batch = get(f"{API}/repos/{repo}/releases?per_page=100&page={page}", token)
        releases += batch
        if len(batch) < 100:
            break
        page += 1
    return info, releases


def published(releases):
    out = [r for r in releases if not r.get("draft") and r.get("published_at")]
    out.sort(key=lambda r: r["published_at"], reverse=True)
    return out


def counted(asset):
    return not asset.get("name", "").lower().endswith(NOT_A_DOWNLOAD)


def stats(info, releases, today):
    rel = published(releases)
    downloads = sum(a.get("download_count", 0) for r in rel for a in r.get("assets", []) if counted(a))
    return {
        "stars": int(info.get("stargazers_count", 0)),
        "downloads": int(downloads),
        "releases": len(rel),
        "as_of": today,
    }


def atom(releases, repo, site):
    rel = published(releases)
    updated = rel[0]["published_at"] if rel else dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    gh = f"https://github.com/{repo}"
    lines = [
        '<?xml version="1.0" encoding="utf-8"?>',
        '<feed xmlns="http://www.w3.org/2005/Atom">',
        "  <title>Cantus releases</title>",
        "  <subtitle>Every release of Cantus, the free, open-source DAW: its version, its date and its notes.</subtitle>",
        f'  <link rel="self" type="application/atom+xml" href="{site}/releases.xml"/>',
        f'  <link rel="alternate" type="text/html" href="{gh}/releases"/>',
        f"  <id>{site}/releases.xml</id>",
        f"  <updated>{escape(updated)}</updated>",
        "  <author><name>Luis Ángel Almazán</name></author>",
        f'  <icon>{site}/favicon-32.png</icon>',
    ]
    for r in rel:
        title = (r.get("name") or "").strip() or r.get("tag_name", "")
        url = r.get("html_url") or f"{gh}/releases/tag/{r.get('tag_name', '')}"
        kind = "Pre-release" if r.get("prerelease") else "Release"
        lines += [
            "  <entry>",
            f"    <title>{escape(title)}</title>",
            f'    <link rel="alternate" type="text/html" href="{escape(url)}"/>',
            f"    <id>{escape(url)}</id>",
            f"    <published>{escape(r['published_at'])}</published>",
            f"    <updated>{escape(r['published_at'])}</updated>",
            f"    <summary>{escape(kind)} {escape(r.get('tag_name', ''))}: the notes and the files are on GitHub.</summary>",
            "  </entry>",
        ]
    lines.append("</feed>")
    return "\n".join(lines) + "\n"


def main():
    here = Path(__file__).resolve().parent
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", type=Path, default=here.parent / "assets")
    ap.add_argument("--repo", default=REPO)
    ap.add_argument("--site", default=SITE)
    ap.add_argument("--from-json", type=Path, help="read repo.json and releases.json from this folder instead of GitHub")
    ap.add_argument("--strict", action="store_true", help="fail when GitHub cannot be reached")
    a = ap.parse_args()

    try:
        if a.from_json:
            info = json.loads((a.from_json / "repo.json").read_text(encoding="utf-8"))
            releases = json.loads((a.from_json / "releases.json").read_text(encoding="utf-8"))
        else:
            info, releases = fetch(a.repo, os.environ.get("GITHUB_TOKEN", ""))
    except (urllib.error.URLError, OSError, ValueError) as e:
        print(f"site_data: GitHub not reached ({e}); no stats.json and no releases.xml", file=sys.stderr)
        return 1 if a.strict else 0

    today = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%d")
    a.out.mkdir(parents=True, exist_ok=True)
    s = stats(info, releases, today)
    (a.out / "stats.json").write_text(json.dumps(s, indent=2) + "\n", encoding="utf-8")
    (a.out / "releases.xml").write_text(atom(releases, a.repo, a.site), encoding="utf-8")
    print(f"site_data: {s['stars']} stars, {s['downloads']} downloads over {s['releases']} releases -> {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
