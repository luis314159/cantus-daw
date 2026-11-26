#!/usr/bin/env python3
"""The sound tools' reference sets: fetch, verify, record (round 42, sound-data).

`catalog.toml` names every set (where it comes from, its licence, what may be done with it,
which tools use it, the sub-path under `$CANTUS_REFS` it lives at); `files/<set>.tsv` lists
its files with their sizes, SHA-256 and where each one comes from. No audio is in the
repository. Run with `python -I` (stdlib only):

    python -I tools/sound-refs/fetch.py list                     # every set: size, licence, how
    python -I tools/sound-refs/fetch.py fetch [set|glob ...]     # download what is missing
    python -I tools/sound-refs/fetch.py verify [set|glob ...]    # sizes + SHA-256 of what is there
    python -I tools/sound-refs/fetch.py manual [set ...]         # the hand steps, printed
    python -I tools/sound-refs/fetch.py record <set ...>         # maintainers: TSV from disk

Options: `--refs DIR` (default `$CANTUS_REFS`, else F:\\projects\\cantus-refs), `--quick`
(sizes only), `--packs` (also the Cantus sound packs: copied from the app's own content folder
when it holds them at the manifest's version, else installed by `--cargo`), `--keep-cache`.

How a file arrives, by its `origin` in the TSV (the origin's `kind` in the catalogue):
`url` a plain download (resumed from `.part`); `zip` one member of a remote zip read by HTTP
range requests (or of a zip already in the set, `local`), CRC-checked by zipfile and then
SHA-256-checked here; `tar` a member of a .tar.gz downloaded whole to `$CANTUS_WORK/fetch-cache`
(resumable) and read once; `copy` the same bytes as another catalogued file; `repo` a text
kept in `tools/sound-refs/sources/`; `derived` written by a tool (the set's `derive`
command; verified, never downloaded); `meta` a page saved when the set was first fetched
(not needed by any tool, not fetched again); `manual` placed by hand (`manual` prints how);
`pack` one of Cantus's own sound packs, installed by Cantus's pack code.

The network: a generic User-Agent, the system's proxy and certificates, never a bypass. A
corporate proxy (Zscaler) may quarantine a download for ~10 minutes and answer with an HTML
page meanwhile: every answer is checked (status, type, first bytes: PK, fLaC, RIFF, FORM,
ID3, OggS, %PDF), and a page where data was expected waits `CANTUS_FETCH_WAIT` seconds (60) and
tries again, `CANTUS_FETCH_TRIES` times (20). A site that refuses automated clients (a
browser challenge, HTTP 403) is never worked around: its set is `manual`.
"""
import argparse
import fnmatch
import hashlib
import io
import json
import os
import shutil
import ssl
import subprocess
import sys
import tarfile
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import zipfile
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE))
import cantus_paths  # noqa: E402

CATALOG = HERE / "catalog.toml"
FILES = HERE / "files"
SOURCES = HERE / "sources"
UA = "cantus-sound-refs/1.0 (reference-data fetcher)"
WAIT = float(os.environ.get("CANTUS_FETCH_WAIT", "60"))
TRIES = int(os.environ.get("CANTUS_FETCH_TRIES", "20"))
BLOCK = 4 << 20
COLS = ("path", "size", "sha256", "origin", "member")
# First bytes a file of each extension starts with (ID3 or an MPEG frame for an MP3).
MAGIC = {".zip": [b"PK"], ".flac": [b"fLaC"], ".wav": [b"RIFF", b"RF64"], ".aif": [b"FORM"],
         ".aiff": [b"FORM"], ".mp3": [b"ID3", b"\xff\xfb", b"\xff\xf3", b"\xff\xfa", b"\xff\xf2"],
         ".pdf": [b"%PDF"], ".gz": [b"\x1f\x8b"], ".mat": [b"MATLAB"], ".mid": [b"MThd"],
         ".ogg": [b"OggS"], ".oga": [b"OggS"]}


def say(*a):
    print(*a, flush=True)


# ---------------------------------------------------------------- the catalogue

def load_catalog():
    with open(CATALOG, "rb") as fh:
        cat = tomllib.load(fh)
    sets = cat.get("set", [])
    for s in sets:
        s.setdefault("origins", {})
    return cat, sets


def tsv_path(s):
    return FILES / f"{s['id']}.tsv"


def read_rows(s):
    p = tsv_path(s)
    if not p.exists():
        return []
    rows = []
    for line in p.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        f = line.split("\t")
        f += [""] * (5 - len(f))
        rows.append({"path": f[0], "size": int(f[1]), "sha256": f[2], "origin": f[3], "member": f[4]})
    return rows


def write_rows(s, rows):
    FILES.mkdir(exist_ok=True)
    rows = sorted(rows, key=lambda r: r["path"])
    head = (f"# {s['id']}: the files under $CANTUS_REFS/{s['dir']}/ — written by "
            f"`fetch.py record {s['id']}`; origins in catalog.toml\n# " + "\t".join(COLS) + "\n")
    body = "".join("\t".join(str(r[c]) for c in COLS) + "\n" for r in rows)
    with open(tsv_path(s), "w", encoding="utf-8", newline="\n") as fh:
        fh.write(head + body)


def select(sets, names, default_all=True):
    if not names:
        return list(sets) if default_all else []
    out = []
    for n in names:
        hit = [s for s in sets if fnmatch.fnmatchcase(s["id"], n)]
        if not hit:
            sys.exit(f"no set matches {n!r} (fetch.py list names them)")
        out += [s for s in hit if s not in out]
    return out


def origin_of(s, name):
    if name == "copy":
        return {"kind": "copy"}
    o = s["origins"].get(name)
    if o is None:
        raise KeyError(f"{s['id']}: origin {name!r} is not in catalog.toml")
    return o


def human(n):
    for u in ("B", "kB", "MB", "GB"):
        if n < 1000 or u == "GB":
            return f"{n:.0f} {u}" if u == "B" else f"{n:.1f} {u}"
        n /= 1000


# ---------------------------------------------------------------- hashing, with a stamp cache

class Stamps:
    """SHA-256 of files already hashed, keyed by (size, mtime): a second run hashes nothing
    that did not change. Kept at `$CANTUS_REFS/.sound-refs-stamps.tsv`."""

    def __init__(self, root):
        self.p = Path(root) / ".sound-refs-stamps.tsv"
        self.d, self.dirty = {}, 0
        if self.p.exists():
            for line in self.p.read_text(encoding="utf-8").splitlines():
                f = line.split("\t")
                if len(f) == 4:
                    self.d[f[0]] = (int(f[1]), int(f[2]), f[3])

    def sha(self, path, rel):
        st = path.stat()
        hit = self.d.get(rel)
        if hit and hit[0] == st.st_size and hit[1] == st.st_mtime_ns:
            return hit[2]
        h = sha256_file(path)
        self.d[rel] = (st.st_size, st.st_mtime_ns, h)
        self.dirty += 1
        if self.dirty >= 500:
            self.save()
        return h

    def save(self):
        if not self.dirty:
            return
        self.p.parent.mkdir(parents=True, exist_ok=True)
        tmp = self.p.with_suffix(".tmp")
        with open(tmp, "w", encoding="utf-8", newline="\n") as fh:
            for k, (n, t, h) in sorted(self.d.items()):
                fh.write(f"{k}\t{n}\t{t}\t{h}\n")
        os.replace(tmp, self.p)
        self.dirty = 0


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        while True:
            b = fh.read(1 << 20)
            if not b:
                return h.hexdigest()
            h.update(b)


def crc32_file(path):
    c = 0
    with open(path, "rb") as fh:
        while True:
            b = fh.read(1 << 20)
            if not b:
                return c & 0xFFFFFFFF
            c = zlib.crc32(b, c)


def magic_ok(name, head):
    want = MAGIC.get(Path(name).suffix.lower())
    if want is None:
        return not looks_html(head) or Path(name).suffix.lower() in (".html", ".htm", ".xml", ".txt", ".md")
    return any(head.startswith(m) for m in want)


def looks_html(head):
    h = head.lstrip()[:15].lower()
    return h.startswith(b"<!doctype") or h.startswith(b"<html") or h.startswith(b"<head")


# ---------------------------------------------------------------- the network

class Wait(Exception):
    """An answer that is not the data (a proxy's page, a busy server): wait, then try again."""


class Refused(Exception):
    """An answer that waiting will not change (404, a browser challenge): stop."""


class TlsRefused(Refused):
    """Python could not verify the server's certificate: never retried, never bypassed."""


def request(url, start=None, end=None):
    h = {"User-Agent": UA}
    if start is not None:
        h["Range"] = f"bytes={start}-" + ("" if end is None else str(end))
    try:
        r = urllib.request.urlopen(urllib.request.Request(url, headers=h), timeout=120)
    except urllib.error.HTTPError as e:
        if e.code in (408, 425, 429, 500, 502, 503, 504):
            raise Wait(f"HTTP {e.code}")
        if e.code == 416:
            raise
        body = e.read(2048)
        challenge = b"challenge" in body.lower() or b"cloudflare" in body.lower()
        raise Refused(f"HTTP {e.code}{' (a browser challenge: fetch it by hand)' if challenge else ''}: {url}")
    except urllib.error.URLError as e:
        if isinstance(e.reason, ssl.SSLError):
            raise TlsRefused(f"TLS: {e.reason} ({url}); the check is never bypassed")
        raise Wait(f"{type(e).__name__}: {e}")
    except ssl.SSLError as e:
        raise TlsRefused(f"TLS: {e} ({url}); the check is never bypassed")
    except (TimeoutError, ConnectionError, OSError) as e:
        raise Wait(f"{type(e).__name__}: {e}")
    ctype = (r.headers.get("Content-Type") or "").lower()
    if start is not None and r.status == 200 and "html" in ctype:
        r.close()
        raise Wait("an HTML page where data was expected (a proxy holding the file?)")
    return r


def retrying(what, fn):
    for i in range(TRIES):
        try:
            return fn()
        except Wait as e:
            if i == TRIES - 1:
                raise Refused(f"{what}: still failing after {TRIES} tries ({e})")
            say(f"  wait {WAIT:.0f} s ({e}) — {what} [{i + 1}/{TRIES}]")
            time.sleep(WAIT)


class RangeFile(io.RawIOBase):
    """A remote file read by HTTP range requests, seekable: zipfile reads a 14 GB zip's
    central directory and the members it is asked for, nothing else."""

    def __init__(self, url):
        self.url, self.pos, self.cache = url, 0, {}

        def probe():
            with request(url, 0, 0) as r:
                cr = r.headers.get("Content-Range")
                if r.status != 206 or not cr:
                    raise Refused(f"{url}: the server does not answer range requests")
                return int(cr.rsplit("/", 1)[1])
        self.size = retrying(url, probe)

    def readable(self):
        return True

    def seekable(self):
        return True

    def tell(self):
        return self.pos

    def seek(self, off, whence=0):
        self.pos = off if whence == 0 else self.pos + off if whence == 1 else self.size + off
        return self.pos

    def _block(self, i):
        if i not in self.cache:
            a = i * BLOCK
            b = min(self.size, a + BLOCK) - 1

            def get():
                with request(self.url, a, b) as r:
                    if r.status != 206:
                        raise Wait(f"HTTP {r.status} to a range request")
                    data = r.read()
                if len(data) != b - a + 1:
                    raise Wait(f"short read ({len(data)} of {b - a + 1} bytes)")
                return data
            if len(self.cache) >= 6:
                self.cache.pop(next(iter(self.cache)))
            self.cache[i] = retrying(f"{self.url} @{a}", get)
        return self.cache[i]

    def readinto(self, buf):
        n = min(len(buf), self.size - self.pos)
        got = 0
        while got < n:
            i, off = divmod(self.pos, BLOCK)
            chunk = self._block(i)[off:off + n - got]
            buf[got:got + len(chunk)] = chunk
            got += len(chunk)
            self.pos += len(chunk)
        return got


class CurlRange(RangeFile):
    """`RangeFile`'s reads through curl, which verifies with the system's certificate store —
    for a site whose chain Python's own store refuses (drumgizmo.org on 2026-11-26: "certificate
    has expired" in Python, verified by curl/Schannel). Never `-k`."""

    def __init__(self, url):
        self.url, self.pos, self.cache = url, 0, {}
        head = subprocess.run(["curl", "-sSIL", "--fail", url], capture_output=True, text=True,
                              check=True).stdout
        sizes = [int(l.split(":", 1)[1]) for l in head.splitlines()
                 if l.lower().startswith("content-length:")]
        if not sizes or "text/html" in head.lower():
            raise Refused(f"{url}: no size (an HTML page?)")
        self.size = sizes[-1]

    def _block(self, i):
        if i not in self.cache:
            a = i * BLOCK
            b = min(self.size, a + BLOCK) - 1

            def get():
                r = subprocess.run(["curl", "-sSL", "--fail", "-r", f"{a}-{b}", self.url],
                                   capture_output=True)
                if r.returncode != 0:
                    raise Wait(f"curl exit {r.returncode}: {r.stderr[:200]!r}")
                if len(r.stdout) != b - a + 1 or looks_html(r.stdout[:512]):
                    raise Wait(f"short or HTML read ({len(r.stdout)} of {b - a + 1})")
                return r.stdout
            if len(self.cache) >= 6:
                self.cache.pop(next(iter(self.cache)))
            self.cache[i] = retrying(f"{self.url} @{a}", get)
        return self.cache[i]


def open_remote(url):
    """A remote zip by range requests: Python's own TLS check first, curl after (round 42)."""
    try:
        return RangeFile(url)
    except TlsRefused as e:
        print(f"  {e}; reading through curl (the system's store)", flush=True)
        return CurlRange(url)


def download(url, dest, size=None, magic_name=None):
    """`url` into `dest`, resumed from `dest.part`; the first bytes checked against the
    file's kind (an HTML page instead of data is a proxy's: wait and retry)."""
    dest = Path(dest)
    part = dest.with_name(dest.name + ".part")
    dest.parent.mkdir(parents=True, exist_ok=True)
    name = magic_name or dest.name

    def once():
        have = part.stat().st_size if part.exists() else 0
        if size is not None and have > size:
            part.unlink()
            have = 0
        if size is not None and have == size:
            return
        try:
            r = request(url, have if have else None)
        except urllib.error.HTTPError as e:  # 416: the part is whole, or wrong
            part.unlink(missing_ok=True)
            raise Wait(f"HTTP {e.code}, starting again")
        with r:
            mode = "ab" if have and r.status == 206 else "wb"
            first = r.read(64 * 1024)
            if mode == "wb" and (looks_html(first) and not name.lower().endswith((".html", ".htm"))
                                 or not magic_ok(name, first)):
                raise Wait(f"{name} began {first[:12]!r}: not the file (a proxy's page?)")
            with open(part, mode) as fh:
                fh.write(first)
                while True:
                    b = r.read(1 << 20)
                    if not b:
                        break
                    fh.write(b)
        if size is not None and part.stat().st_size != size:
            raise Wait(f"{part.stat().st_size} of {size} bytes, resuming")
    try:
        retrying(url, once)
    except TlsRefused as e:
        # Python's OpenSSL may judge a chain the system's own store accepts (an expired
        # cross-signed root among Windows' certificates): curl, verifying with the system's
        # store (schannel on Windows), is asked instead — still a verified connection.
        if not shutil.which("curl"):
            raise
        say(f"  {e}\n  trying curl (it verifies TLS with the system's store) …")
        retrying(url, lambda: curl_once(url, part, size, name))
    os.replace(part, dest)


def curl_once(url, part, size, name):
    have = part.stat().st_size if part.exists() else 0
    if size is not None and have == size:
        return
    cmd = ["curl", "-sSL", "--fail", "-A", UA, "-m", "3600", "-o", str(part)]
    if have:
        cmd[1:1] = ["-C", "-"]
    rc = subprocess.call(cmd + [url])
    if rc in (22, 35, 51, 58, 60, 77, 83, 90, 91):  # HTTP error or a certificate refused
        part.unlink(missing_ok=True)
        raise Refused(f"curl exit {rc} for {url} (an HTTP error or the certificate refused)")
    if rc:
        raise Wait(f"curl exit {rc}")
    with open(part, "rb") as fh:
        first = fh.read(64)
    if not have and not magic_ok(name, first):
        part.unlink()
        raise Wait(f"{name} began {first[:12]!r}: not the file (a proxy's page?)")
    if size is not None and part.stat().st_size != size:
        raise Wait(f"{part.stat().st_size} of {size} bytes, resuming")


# ---------------------------------------------------------------- fetching

class Ctx:
    def __init__(self, a, sets):
        self.root = Path(a.refs)
        self.quick = getattr(a, "quick", False)
        self.keep_cache = getattr(a, "keep_cache", False)
        self.stamps = Stamps(self.root)
        self.sets = sets
        self.by_path = {}
        for s in sets:
            for r in read_rows(s):
                self.by_path[f"{s['dir']}/{r['path']}"] = (s, r)
        self.notes = []
        self.caches = set()

    def dest(self, s, r):
        return self.root / s["dir"] / r["path"]

    def rel(self, s, r):
        return f"{s['dir']}/{r['path']}"

    def want_sha(self, s, r):
        if r["origin"] == "repo" or origin_of(s, r["origin"]).get("kind") == "repo":
            src = SOURCES / s["dir"] / r["path"]
            return sha256_file(src) if src.exists() else r["sha256"]
        return r["sha256"]

    def ok(self, s, r):
        p = self.dest(s, r)
        if not p.is_file():
            return False
        if loose(s, r):  # a live page: whatever it says today
            return True
        want = self.want_sha(s, r)
        if want != r["sha256"]:  # a repo text edited since it was recorded: its bytes decide
            return self.stamps.sha(p, self.rel(s, r)) == want
        if p.stat().st_size != r["size"]:
            return False
        return self.quick or self.stamps.sha(p, self.rel(s, r)) == want

    def settle(self, s, r, tmp):
        """`tmp` holds the file: its SHA-256 decides whether it becomes the file."""
        h = sha256_file(tmp)
        want = self.want_sha(s, r)
        if h != want and loose(s, r):
            self.notes.append(f"{self.rel(s, r)}: a live page, it changed since it was recorded (kept)")
        elif h != want:
            bad = Path(str(tmp) + ".bad")
            os.replace(tmp, bad)
            raise Refused(f"{self.rel(s, r)}: SHA-256 {h[:12]}…, the catalogue says {want[:12]}… (kept as {bad.name})")
        dest = self.dest(s, r)
        os.replace(tmp, dest)
        self.stamps.d[self.rel(s, r)] = (dest.stat().st_size, dest.stat().st_mtime_ns, h)
        self.stamps.dirty += 1


def loose(s, r):
    """A live page (an index, a licence page): its bytes may change; it is never a failure."""
    return r["origin"] != "copy" and bool(origin_of(s, r["origin"]).get("loose"))


def url_of(o, r):
    if o.get("url") and not o.get("base"):
        return o["url"]
    if r["member"].startswith(("http://", "https://")):
        return r["member"]
    return o["base"] + urllib.parse.quote(r["member"])


def part_of(dest):
    dest.parent.mkdir(parents=True, exist_ok=True)
    return dest.with_name(dest.name + ".part")


def fetch_rows(ctx, s, rows, quiet=False):
    """Bring `rows` of set `s` into place, origin by origin; what cannot be is noted."""
    groups = {}
    for r in rows:
        groups.setdefault(r["origin"], []).append(r)
    order = [o for o in s["origins"] if o in groups] + [o for o in groups if o not in s["origins"]]
    for name in order:
        todo = [r for r in groups[name] if not ctx.ok(s, r)]
        if not todo:
            continue
        o = origin_of(s, name)
        kind = o.get("kind")
        if not quiet:
            say(f"  {s['id']}: {len(todo)} file(s) from {name} ({kind})")
        try:
            if kind == "url":
                for r in todo:
                    d = ctx.dest(s, r)
                    download(url_of(o, r), part_of(d), r["size"], magic_name=d.name)
                    ctx.settle(s, r, part_of(d))
            elif kind == "zip":
                fetch_zip(ctx, s, o, todo)
            elif kind == "tar":
                fetch_tar(ctx, s, o, todo)
            elif kind == "copy":
                for r in todo:
                    src = ctx.by_path.get(r["member"])
                    if src is None:
                        raise Refused(f"{ctx.rel(s, r)}: its source {r['member']} is in no set")
                    fetch_rows(ctx, src[0], [src[1]], quiet=True)
                    shutil.copyfile(ctx.dest(*src), part_of(ctx.dest(s, r)))
                    ctx.settle(s, r, part_of(ctx.dest(s, r)))
            elif kind == "repo":
                for r in todo:
                    shutil.copyfile(SOURCES / s["dir"] / r["path"], part_of(ctx.dest(s, r)))
                    ctx.settle(s, r, part_of(ctx.dest(s, r)))
            elif kind == "derived":
                ctx.notes.append(f"{s['id']}: {len(todo)} derived file(s) missing — run: {o.get('command', s.get('derive', '?'))}")
            elif kind == "meta":
                pass  # saved pages, not needed by any tool
            elif kind == "manual":
                ctx.notes.append(f"{s['id']}: {len(todo)} file(s) to place by hand — fetch.py manual {s['id']}")
            elif kind == "pack":
                ctx.notes.append(f"{s['id']}: {len(todo)} pack file(s) missing — fetch.py fetch {s['id']} --packs")
            else:
                raise Refused(f"unknown origin kind {kind!r}")
        except Refused as e:
            ctx.notes.append(f"{s['id']}: {e}")
            say(f"  FAILED: {e}")
        finally:
            ctx.stamps.save()


def zip_source(ctx, s, o):
    if o.get("local"):
        local = ctx.root / s["dir"] / o["local"]
        row = next((r for r in read_rows(s) if r["path"] == o["local"]), None)
        if row is not None:
            fetch_rows(ctx, s, [row], quiet=True)
        if local.exists():
            return open(local, "rb")
    return open_remote(o["url"])


def fetch_zip(ctx, s, o, todo):
    src = zip_source(ctx, s, o)
    with src, zipfile.ZipFile(src) as zf:
        for r in todo:
            d = ctx.dest(s, r)
            tmp = part_of(d)
            for i in range(3):
                try:
                    with zf.open(r["member"]) as f, open(tmp, "wb") as out:
                        shutil.copyfileobj(f, out, 1 << 20)
                    break
                except zipfile.BadZipFile as e:  # a CRC failure: read it again
                    if i == 2:
                        raise Refused(f"{r['member']}: {e}")
                    say(f"  {r['member']}: {e}; again")
            ctx.settle(s, r, tmp)


def fetch_tar(ctx, s, o, todo):
    name = o.get("cache_name") or Path(urllib.parse.urlparse(o["url"]).path).name
    if name in ("content", ""):
        name = s["id"] + ".tar.gz"
    cache = cantus_paths.work("fetch-cache", name)
    if not cache.exists():
        say(f"  downloading {o['url']} ({human(o.get('size', 0))}) into {cache} …")
        download(o["url"], cache, o.get("size"), magic_name="x.gz")
    if o.get("md5"):
        h = hashlib.md5()
        with open(cache, "rb") as fh:
            for b in iter(lambda: fh.read(1 << 20), b""):
                h.update(b)
        if h.hexdigest() != o["md5"]:
            cache.rename(str(cache) + ".bad")
            raise Refused(f"{name}: MD5 {h.hexdigest()}, the record says {o['md5']}")
    want = {}
    for r in todo:
        want.setdefault(r["member"], []).append(r)
    with tarfile.open(cache, "r:gz") as tf:
        for m in tf:
            if not m.isfile():
                continue
            hits = want.get(m.name) or want.get(Path(m.name).name)
            if not hits:
                continue
            data = tf.extractfile(m)
            for r in hits:
                tmp = part_of(ctx.dest(s, r))
                with open(tmp, "wb") as out:
                    shutil.copyfileobj(data, out, 1 << 20)
                data.seek(0)
                ctx.settle(s, r, tmp)
    missing = [r for r in todo if not ctx.ok(s, r)]
    if missing:
        raise Refused(f"{len(missing)} member(s) not in {name}, e.g. {missing[0]['member']}")
    ctx.caches.add(cache)  # another set may read the same archive in this run


# ---------------------------------------------------------------- Cantus's own sound packs

def manifest_versions():
    m = json.loads((REPO / "crates" / "cantus-content" / "manifest" / "content.json").read_text(encoding="utf-8"))
    return {p["id"]: p.get("version") for p in m["packs"]}


def app_content_dirs():
    """Where the app keeps its packs (`cantus_ui::app::data_dir()/Content`)."""
    out = []
    for v in ("CANTUS_DATA_DIR", "LOCALAPPDATA"):
        if os.environ.get(v):
            out.append(Path(os.environ[v]) / ("" if v == "CANTUS_DATA_DIR" else "Cantus") / "Content")
    home = Path.home()
    out += [home / "Library/Application Support/Cantus/Content",
            Path(os.environ.get("XDG_DATA_HOME") or home / ".local/share") / "cantus" / "Content"]
    return [p for p in out if p.is_dir()]


def pack_version(d):
    try:
        return json.loads((d / "cantus-pack.json").read_text(encoding="utf-8")).get("version")
    except (OSError, ValueError):
        return None


def fetch_packs(ctx, s, a):
    want = manifest_versions()
    content = ctx.root / s["dir"] / s.get("content", "content")
    missing = []
    for p in s["packs"]:
        if pack_version(content / p) == want.get(p):
            continue
        src = next((d / p for d in app_content_dirs() if pack_version(d / p) == want.get(p)), None)
        if src is not None and a.packs:
            say(f"  {p}: copying the app's own install ({src}) …")
            shutil.copytree(src, content / p, dirs_exist_ok=True)
            continue
        missing.append(p)
    if not missing:
        return
    cmd = s["install"].format(content=content.as_posix(), dir=(ctx.root / s["dir"]).as_posix(),
                              packs=",".join(missing), work=cantus_paths.work().as_posix())
    if a.cargo:
        say(f"  installing {', '.join(missing)} through Cantus's pack code:\n    {cmd}")
        import subprocess
        rc = subprocess.call(["bash", "-c", cmd], cwd=REPO)
        if rc:
            ctx.notes.append(f"{s['id']}: the install exited {rc}")
    else:
        ctx.notes.append(f"{s['id']}: pack(s) {', '.join(missing)} not installed at the manifest's "
                         f"version — run (from the repository, cargo needed):\n    {cmd}\n  or "
                         f"fetch.py fetch {s['id']} --packs --cargo")


# ---------------------------------------------------------------- verify

def verify_set(ctx, s, extras=True, only=()):
    rows = [r for r in read_rows(s) if not only or any(fnmatch.fnmatchcase(r["path"], g) for g in only)]
    if not rows and (s.get("fetch") == "manual" or s.get("expect")):
        d = ctx.root / s["dir"]
        placed = [f for f in s.get("expect", []) if (d / f).is_file()]
        say(f"--  {s['id']:<40} by hand, nothing recorded"
            + (f" ({len(placed)} of {len(s['expect'])} expected files placed: fetch.py record {s['id']})"
               if s.get("expect") else "") + f" — fetch.py manual {s['id']}")
        return 0
    count = {"ok": 0, "missing": 0, "bad": 0, "absent-meta": 0, "missing-derived": 0}
    bad = []
    for r in rows:
        kind = origin_of(s, r["origin"]).get("kind")
        p = ctx.dest(s, r)
        if not p.is_file():
            key = "absent-meta" if kind == "meta" else "missing-derived" if kind == "derived" else "missing"
            count[key] += 1
            if key == "missing":
                bad.append(f"missing {r['path']}")
        elif ctx.ok(s, r):
            count["ok"] += 1
        else:
            count["bad"] += 1
            bad.append(f"differs {r['path']} ({p.stat().st_size} bytes, want {r['size']})")
    known = {r["path"] for r in rows}
    extra = []
    d = ctx.root / s["dir"]
    if extras and d.is_dir() and not s.get("shared_dir"):
        ign = s.get("ignore", [])
        for f in walk(d):
            rel = f.relative_to(d).as_posix()
            if rel not in known and not any(fnmatch.fnmatchcase(rel, g) for g in ign) \
                    and not rel.endswith((".part", ".bad")) and not under_other_set(ctx, s, rel):
                extra.append(rel)
    ctx.stamps.save()
    size = sum(r["size"] for r in rows)
    flag = "OK " if not count["missing"] and not count["bad"] else "BAD"
    say(f"{flag} {s['id']:<40} {count['ok']:>6} ok / {len(rows):<6} {human(size):>9}"
        + "".join(f"  {k} {v}" for k, v in count.items() if v and k != "ok")
        + (f"  extra {len(extra)}" if extra else ""))
    for b in bad[:8]:
        say(f"      {b}")
    if len(bad) > 8:
        say(f"      … {len(bad) - 8} more")
    for e in extra[:5]:
        say(f"      extra (not catalogued): {e}")
    return count["missing"] + count["bad"]


def under_other_set(ctx, s, rel):
    full = f"{s['dir']}/{rel}"
    return any(o is not s and full.startswith(o["dir"] + "/") and len(o["dir"]) > len(s["dir"])
               for o in ctx.sets)


def walk(d):
    for dp, dn, fn in os.walk(d):
        dn.sort()
        for f in sorted(fn):
            p = Path(dp) / f
            if not p.is_symlink():
                yield p


# ---------------------------------------------------------------- record (maintainers)

def zip_index(ctx, s, o):
    """(basename, size, CRC-32) → member names of a zip; a remote one's index is cached."""
    key = hashlib.sha1((o.get("local") or o["url"]).encode()).hexdigest()[:16]
    cache = cantus_paths.work("fetch-cache", f"zipindex-{key}.json")
    if not o.get("local") and cache.exists():
        raw = json.loads(cache.read_text(encoding="utf-8"))
    else:
        say(f"  reading the central directory of {o.get('local') or o['url']} …")
        src = zip_source(ctx, s, o)
        with src, zipfile.ZipFile(src) as zf:
            raw = [[i.filename, i.file_size, i.CRC] for i in zf.infolist() if not i.is_dir()]
        if not o.get("local"):
            cache.parent.mkdir(parents=True, exist_ok=True)
            cache.write_text(json.dumps(raw), encoding="utf-8")
    idx = {}
    for name, size, crc in raw:
        idx.setdefault((Path(name).name.lower(), size, crc), []).append(name)
    return idx


def best_member(names, rel):
    def tail(n):
        a, b = Path(n).parts[::-1], Path(rel).parts[::-1]
        k = 0
        while k < min(len(a), len(b)) and a[k].lower() == b[k].lower():
            k += 1
        return k
    return max(names, key=tail)


def resolve(ctx, s, rel, f, size, zidx):
    for name, o in s["origins"].items():
        kind = o["kind"]
        globs = o.get("match", ["*"] if kind in ("zip", "tar") else [])
        if not any(fnmatch.fnmatchcase(rel, g) for g in globs):
            continue
        member = ""
        if kind == "url" and o.get("base"):
            strip = o.get("strip", "")
            member = o.get("prefix", "") + (rel[len(strip):] if rel.startswith(strip) else rel)
        elif kind == "zip":
            if name not in zidx:
                zidx[name] = zip_index(ctx, s, o)
            hit = zidx[name].get((Path(rel).name.lower(), size, crc32_file(f)))
            if not hit:
                continue
            member = best_member(hit, rel)
        elif kind == "tar":
            member = Path(rel).name
        elif kind == "repo":
            dst = SOURCES / s["dir"] / rel
            if not dst.exists():
                dst.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(f, dst)
        elif kind == "pack":
            member = rel
        return name, member
    return "", ""


def record(ctx, targets, fresh=False):
    prev = {s["id"]: {r["path"]: r for r in read_rows(s)} for s in ctx.sets}
    done = {}
    for s in targets:
        d = ctx.root / s["dir"]
        if not d.is_dir():
            say(f"  {s['id']}: {d} is not there; nothing recorded")
            continue
        ign, zidx, rows = s.get("ignore", []), {}, []
        for f in walk(d):
            rel = f.relative_to(d).as_posix()
            if any(fnmatch.fnmatchcase(rel, g) for g in ign) or rel.endswith((".part", ".bad")) \
                    or under_other_set(ctx, s, rel):
                continue
            size = f.stat().st_size
            sha = ctx.stamps.sha(f, f"{s['dir']}/{rel}")
            row = {"path": rel, "size": size, "sha256": sha, "origin": "", "member": ""}
            old = prev[s["id"]].get(rel)
            if not fresh and old and old["sha256"] == sha and (old["origin"] in s["origins"]
                                                                or old["origin"] == "copy"):
                row.update(origin=old["origin"], member=old["member"])
            else:
                row["origin"], row["member"] = resolve(ctx, s, rel, f, size, zidx)
            rows.append(row)
        done[s["id"]] = rows
    index = {}
    for s in ctx.sets:
        for r in done.get(s["id"]) or prev[s["id"]].values():
            if r["origin"] and r["origin"] not in ("?", "copy"):
                index.setdefault(r["sha256"], f"{s['dir']}/{r['path']}")
    for s in targets:
        if s["id"] not in done:
            continue
        rows = done[s["id"]]
        for r in rows:
            if not r["origin"] or r["origin"] == "?":
                src = index.get(r["sha256"])
                if src and src != f"{s['dir']}/{r['path']}":
                    r["origin"], r["member"] = "copy", src
                else:
                    r["origin"] = "?"
        write_rows(s, rows)
        ctx.stamps.save()
        by = {}
        for r in rows:
            by[r["origin"]] = by.get(r["origin"], 0) + 1
        say(f"  {s['id']}: {len(rows)} files, {human(sum(r['size'] for r in rows))}: "
            + ", ".join(f"{k} {v}" for k, v in sorted(by.items())))
        for r in [r for r in rows if r["origin"] == "?"][:10]:
            say(f"      UNRESOLVED {r['path']}")


# ---------------------------------------------------------------- list, manual, main

def how(s):
    kinds = {origin_of(s, r["origin"]).get("kind") for r in read_rows(s)} - {None}
    if s.get("packs"):
        return "pack"
    if s.get("fetch"):
        return s["fetch"]
    if kinds <= {"derived", "repo", "copy"} and "derived" in kinds:
        return "derived"
    return "auto+manual" if "manual" in kinds else "auto"


def cmd_list(ctx, sets):
    say(f"$CANTUS_REFS = {ctx.root}\n")
    say(f"{'set':<40} {'files':>6} {'size':>9}  {'fetch':<11} {'licence':<22} use")
    tot = 0
    for s in sets:
        rows = read_rows(s)
        n = sum(r["size"] for r in rows)
        tot += n
        say(f"{s['id']:<40} {len(rows):>6} {human(n):>9}  {how(s):<11} {s.get('licence', '?'):<22} {s.get('use', '?')}")
    say(f"\n{'total':<40} {'':>6} {human(tot):>9}")


def cmd_manual(ctx, sets):
    for s in sets:
        kinds = {origin_of(s, r["origin"]).get("kind") for r in read_rows(s)}
        if not (s.get("manual") or "manual" in kinds):
            continue
        d = ctx.root / s["dir"]
        say(f"== {s['id']} → {d}\n{s.get('manual', '').strip()}\n")
        for r in read_rows(s):
            if origin_of(s, r["origin"]).get("kind") == "manual":
                mark = "present" if ctx.ok(s, r) else "MISSING"
                say(f"   {mark:<8} {r['path']}  ({r['size']} bytes, SHA-256 {r['sha256'][:16]}…)")
        for f in s.get("expect", []):
            say(f"   {'present' if (d / f).is_file() else 'MISSING':<8} {f}  (checksum recorded once placed: fetch.py record {s['id']})")
        say("")


def main():
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("cmd", choices=["list", "fetch", "verify", "manual", "record"])
    ap.add_argument("sets", nargs="*", help="set ids or globs (study-*); none: every set")
    ap.add_argument("--refs", default=str(cantus_paths.refs()))
    ap.add_argument("--quick", action="store_true", help="verify sizes only")
    ap.add_argument("--packs", action="store_true", help="also Cantus's sound packs (from the app's folder)")
    ap.add_argument("--cargo", action="store_true", help="install missing packs through cargo")
    ap.add_argument("--keep-cache", action="store_true", help="keep downloaded archives")
    ap.add_argument("--fresh", action="store_true", help="record: resolve every origin again")
    ap.add_argument("--only", action="append", default=[], metavar="GLOB",
                    help="fetch/verify: only the files whose path in the set matches (repeatable)")
    a = ap.parse_args()
    _, sets = load_catalog()
    ctx = Ctx(a, sets)
    if a.cmd == "list":
        return cmd_list(ctx, select(sets, a.sets))
    if a.cmd == "manual":
        return cmd_manual(ctx, select(sets, a.sets))
    if a.cmd == "record":
        if not a.sets:
            sys.exit("record: name the set(s) to record")
        return record(ctx, select(sets, a.sets), a.fresh)
    chosen = select(sets, a.sets)
    if not a.sets and a.cmd == "fetch":  # by default: what can arrive without a hand
        chosen = [s for s in chosen if s.get("fetch") != "manual" and (a.packs or not s.get("packs"))]
    bad = 0
    if a.cmd == "fetch":
        say(f"fetching into {ctx.root}")
        for s in chosen:
            if s.get("packs"):
                fetch_packs(ctx, s, a)
            rows = [r for r in read_rows(s) if not a.only or any(fnmatch.fnmatchcase(r["path"], g) for g in a.only)]
            fetch_rows(ctx, s, rows)
        say("")
    for s in chosen:
        bad += verify_set(ctx, s, extras=a.cmd == "verify" and not a.only, only=a.only)
    if not a.keep_cache:
        for c in ctx.caches:
            c.unlink(missing_ok=True)
    for n in ctx.notes:
        say(f"note: {n}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
