"""Git access for tools/public/scan.py: read-only plumbing, nothing is ever written."""

import re
import subprocess
import threading

# A ref list may hold these options; anything else starting with "-" is refused, so that a
# stray option can never reach `git rev-list` and leave the scan with nothing to scan.
REF_OPTIONS = {"--all", "--tags", "--branches"}


class GitError(Exception):
    pass


def git(repo, *args):
    p = subprocess.run(["git", "-C", repo, *args], capture_output=True)
    if p.returncode:
        raise GitError("git %s: %s" % (" ".join(args), p.stderr.decode(errors="replace")))
    return p.stdout


def check_refs(refs):
    bad = [r for r in refs if r.startswith("-") and r not in REF_OPTIONS]
    if bad:
        raise GitError("refusing ref option(s) %s: only %s, or ref names"
                       % (" ".join(bad), ", ".join(sorted(REF_OPTIONS))))
    return refs or ["--all"]


def list_commits(repo, refs):
    return git(repo, "rev-list", *refs, "--").decode().split()


def tree_entries(repo, commit):
    """(path, blob id) for every file of a commit."""
    raw = git(repo, "ls-tree", "-r", "-z", "--full-tree", commit)
    for rec in raw.split(b"\0"):
        if not rec:
            continue
        meta, path = rec.split(b"\t", 1)
        _mode, kind, oid = meta.split()
        if kind == b"blob":
            yield path.decode("utf-8", "replace"), oid.decode()


def read_blobs(repo, oids):
    """Yield (oid, bytes) for each blob, through one `git cat-file --batch`."""
    proc = subprocess.Popen(["git", "-C", repo, "cat-file", "--batch"],
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    oids = list(oids)

    def feed():  # from a thread, so a large history cannot dead-lock the pipes
        for oid in oids:
            proc.stdin.write((oid + "\n").encode())
        proc.stdin.close()

    threading.Thread(target=feed, daemon=True).start()
    for _ in oids:
        header = proc.stdout.readline().split()
        body = proc.stdout.read(int(header[2]))
        proc.stdout.read(1)  # the newline after each object
        yield header[0].decode(), body
    proc.wait()


def commit_records(repo, refs):
    """(sha, author, committer, message) of every commit of the refs, identities raw (no
    .mailmap: what is published is what the objects hold)."""
    log = git(repo, "log", *refs, "--format=%H%x00%an <%ae>%x00%cn <%ce>%x00%B%x1e", "--")
    for rec in log.decode("utf-8", "replace").split("\x1e"):
        parts = rec.strip("\n").split("\x00")
        if len(parts) == 4:
            yield parts


def tag_records(repo):
    """(tag name, tagger, message) of every annotated tag in the repository."""
    out = git(repo, "for-each-ref", "refs/tags",
              "--format=%(objecttype)%00%(refname:short)%00%(taggername) %(taggeremail)"
              "%00%(contents)%1e")
    for rec in out.decode("utf-8", "replace").split("\x1e"):
        parts = rec.strip("\n").split("\x00")
        if len(parts) == 4 and parts[0] == "tag":
            yield parts[1], parts[2], parts[3]


_RUN = re.compile(rb"[\x20-\x7e\t]{8,}")


def blob_text(data):
    """The text to scan in a blob: UTF-8 text as is; UTF-16 with a byte-order mark decoded;
    any other binary reduced to its printable ASCII runs of 8 characters or more (strings
    inside images, archives, executables). Returns (text, kind)."""
    if data[:2] in (b"\xff\xfe", b"\xfe\xff"):
        return data.decode("utf-16", "replace"), "utf-16"
    if b"\0" not in data:
        return data.decode("utf-8", "replace").replace("\r\n", "\n"), "text"
    return "\n\n".join(m.group(0).decode("ascii") for m in _RUN.finditer(data)), "binary"
