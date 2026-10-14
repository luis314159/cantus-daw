#!/usr/bin/env python3
"""Read-only scan of a git repository's whole history before it is made public (ADR 0049).

Every commit of the chosen refs is listed with `git ls-tree` (a path is caught in whichever
commit held it); every distinct blob is read once with `git cat-file --batch` — text as is,
UTF-16 decoded, other binaries by their printable ASCII runs; commit messages, annotated
tags and the raw author/committer/tagger identities too. Nothing is written to the repo.

FAILING checks (scan_rules.py has the lists):
  paths     third parties' screenshots, any image or PDF under research/, the RVR1960 file
            (case-insensitive; the same regexes as the filter-repo command)
  rvr1960   blobs holding Reina-Valera 1960 text: salted 6-word shingle hashes
            (rvr1960.fingerprints; no Bible text in this repository)
  secrets   high-confidence token/key patterns and secret-shaped file names
  forbid    any entry of the owner's forbid list (a file OUTSIDE the repository: addresses,
            domains, names) in a path, a blob, a commit or tag message, or an identity
Listed for review, never failing: 4-word RVR1960 shingles, low-confidence assignments,
images outside our own picture folders, identities, e-mail addresses, local user paths.

    python3 tools/public/scan.py [--repo DIR] [--forbid FILE | --no-forbid] [--json OUT]
                                 [--quiet] [--refs REF ...]

--refs takes ref names and only --all, --tags, --branches (default --all).
Exit status: 0 clean, 1 a failing finding, 2 the scan could not run (git error, a refused
ref, no commit to scan, no forbid list).
"""

import argparse
import hashlib
import json
import os
import re
import sys
from collections import defaultdict

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import scan_rules as R  # noqa: E402
from scan_git import (GitError, blob_text, check_refs, commit_records, git,  # noqa: E402
                      list_commits, read_blobs, tag_records, tree_entries)
from textnorm import SALT, SHINGLE, SHINGLE_REVIEW, words  # noqa: E402


def load_fingerprints(path):
    if not os.path.exists(path):
        return set()
    with open(path, encoding="utf-8") as f:
        return {line.strip() for line in f if line.strip() and not line.startswith("#")}


def shingle_hits(w, n, fps):
    if not fps:
        return 0
    return sum(hashlib.sha256((SALT + " ".join(w[i:i + n])).encode()).hexdigest()[:20] in fps
               for i in range(len(w) - n + 1))


def redact(s):
    s = s.strip()
    return s if len(s) <= 12 else s[:8] + "...(%d chars)" % len(s)


class Rules:
    def __init__(self, fp6, fp4, forbid):
        self.fp6, self.fp4, self.forbid = fp6, fp4, forbid
        self.paths = [(n, re.compile(p)) for n, p in R.FORBIDDEN_PATHS]
        self.secrets = [(n, re.compile(p)) for n, p in R.SECRET_PATTERNS]
        self.review = [(n, re.compile(p)) for n, p in R.REVIEW_PATTERNS]
        self.cache = {}

    def forbidden_entries(self, text):
        low = text.lower()
        return [i for i, e in enumerate(self.forbid) if e in low]

    def secret_hits(self, text):
        return [(n, redact(m.group(0))) for n, rx in self.secrets for m in rx.finditer(text)]

    def analyse(self, par):
        """Findings of one paragraph, cached: most blobs are near-copies of each other."""
        key = hashlib.sha1(par.encode("utf-8", "surrogatepass")).digest()
        hit = self.cache.get(key)
        if hit is None:
            w = words(par)
            hit = (shingle_hits(w, SHINGLE, self.fp6), shingle_hits(w, SHINGLE_REVIEW, self.fp4),
                   self.secret_hits(par),
                   [(n, redact(m.group(0))) for n, rx in self.review for m in rx.finditer(par)],
                   self.forbidden_entries(par),
                   [m.group(0).lower() for m in R.EMAIL.finditer(par)
                    if not R.EMAIL_IGNORE.search(m.group(0))],
                   [m.group(1) for m in R.USER_PATH.finditer(par)
                    if m.group(1).lower() not in R.USER_IGNORE])
            self.cache[key] = hit
        return hit


def scan(repo, refs, rules):
    commits = list_commits(repo, refs)
    if not commits:
        raise GitError("no commit to scan in %s" % " ".join(refs))
    blob_paths, blob_commits = defaultdict(set), defaultdict(list)
    forbidden, secret_files, images = defaultdict(list), defaultdict(list), {}
    forbid = []
    path_rule = {}
    for c in commits:
        for path, oid in tree_entries(repo, c):
            if c not in blob_commits[oid][-1:]:
                blob_commits[oid].append(c)
            blob_paths[oid].add(path)
            if path not in path_rule:
                path_rule[path] = next((n for n, rx in rules.paths if rx.search(path)), None)
                if R.IMAGE.search(path) and not path_rule[path] and not any(
                        path.startswith(d) for d in R.OWN_IMAGE_FOLDERS):
                    images[path] = c
                for i in rules.forbidden_entries(path):
                    forbid.append({"path": path, "entry": i, "commit": c})
            if path_rule[path]:
                forbidden[path].append(c)
            if R.SECRET_FILES.search(path):
                secret_files[path].append(c)

    rvr, rvr4, secrets, review = [], [], [], []
    emails, users, kinds = defaultdict(set), defaultdict(set), defaultdict(int)
    for oid, data in read_blobs(repo, blob_paths):
        text, kind = blob_text(data)
        kinds[kind] += 1
        paths = sorted(blob_paths[oid])
        where = {"blob": oid, "kind": kind, "paths": paths, "commits": len(blob_commits[oid]),
                 "newest_commit": blob_commits[oid][0]}
        n6 = n4 = 0
        for par in text.split("\n\n"):
            a6, a4, sec, rev, fb, ems, us = rules.analyse(par)
            n6, n4 = n6 + a6, n4 + a4
            secrets.extend(dict(where, rule=r, match=m) for r, m in sec)
            review.extend(dict(where, rule=r, match=m) for r, m in rev)
            forbid.extend(dict(where, entry=i) for i in fb)
            for e in ems:
                emails[e].update(paths)
            for u in us:
                users[u].update(paths)
        if n6:
            rvr.append(dict(where, hits=n6))
        elif n4:
            rvr4.append(dict(where, hits=n4))

    identities = defaultdict(int)

    def identity(role, who, where):
        identities["%-9s %s" % (role, who)] += 1
        forbid.extend({"identity": role, "where": where, "entry": i}
                      for i in rules.forbidden_entries(who))

    for sha, author, committer, body in commit_records(repo, refs):
        identity("author", author, sha)
        identity("committer", committer, sha)
        secrets.extend({"commit_message": sha, "rule": r, "match": m}
                       for r, m in rules.secret_hits(body))
        forbid.extend({"commit_message": sha, "entry": i}
                      for i in rules.forbidden_entries(body))
    tags = 0
    for name, tagger, body in tag_records(repo):
        tags += 1
        identity("tagger", tagger, name)
        secrets.extend({"tag_message": name, "rule": r, "match": m}
                       for r, m in rules.secret_hits(body))
        forbid.extend({"tag_message": name, "entry": i} for i in rules.forbidden_entries(body))

    def per_path(d):
        return {p: {"commits": len(cs), "newest": cs[0], "oldest": cs[-1]}
                for p, cs in sorted(d.items())}

    return {
        "repo": os.path.abspath(repo), "refs": refs, "commits": len(commits),
        "blobs": len(blob_paths), "blob_kinds": dict(kinds), "annotated_tags": tags,
        "forbidden_paths": {p: dict(v, rule=path_rule[p])
                            for p, v in per_path(forbidden).items()},
        "rvr1960": rvr, "secrets": secrets, "secret_files": per_path(secret_files),
        "forbid": forbid, "forbid_entries": len(rules.forbid),
        "rvr1960_4": rvr4, "review": review,
        "images_outside_own_folders": {p: c for p, c in sorted(images.items())},
        "identities": dict(sorted(identities.items())),
        "emails_in_files": {e: sorted(ps)[:10] for e, ps in sorted(emails.items())},
        "user_paths_in_files": {u: sorted(ps)[:10] for u, ps in sorted(users.items())},
    }


def forbid_list(a, repo):
    """The owner's forbid list, which must exist and live outside the repository."""
    if a.no_forbid:
        print("warning: --no-forbid: no forbid list checked", file=sys.stderr)
        return []
    path = a.forbid or os.environ.get("CANTUS_FORBID") or R.DEFAULT_FORBID
    if not os.path.isfile(path):
        raise GitError("no forbid list at %s: pass --forbid FILE (kept outside the repository)"
                       " or --no-forbid explicitly" % path)
    top = os.path.realpath(git(repo, "rev-parse", "--show-toplevel").decode().strip())
    real = os.path.realpath(path)
    if os.path.normcase(real).startswith(os.path.normcase(top) + os.sep):
        raise GitError("the forbid list %s is inside the repository: keep it outside" % path)
    return R.load_forbid(path)


def main():
    from scan_report import report
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--repo", default=".")
    ap.add_argument("--refs", nargs="*", default=[],
                    help="ref names, or --all/--tags/--branches (default: --all)")
    ap.add_argument("--forbid", help="the forbid list (default: $CANTUS_FORBID, else %s)"
                    % R.DEFAULT_FORBID)
    ap.add_argument("--no-forbid", action="store_true", help="explicitly check no forbid list")
    ap.add_argument("--json", help="also write the findings as JSON here")
    ap.add_argument("--quiet", action="store_true", help="print only the failing lists")
    ap.add_argument("--filter-repo-args", action="store_true",
                    help="print the --path-regex arguments for git filter-repo and exit")
    # Let "--refs --all" and "--refs --tags" through argparse as values.
    argv = sys.argv[1:]
    if "--refs" in argv:
        i = argv.index("--refs")
        refs, argv = argv[i + 1:], argv[:i]
    else:
        refs = []
    a = ap.parse_args(argv)
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    if a.filter_repo_args:
        print(" ".join("--path-regex '%s'" % p for _, p in R.FORBIDDEN_PATHS))
        return 0
    try:
        refs = check_refs(refs)
        forbid = forbid_list(a, a.repo)
        fp6 = load_fingerprints(os.path.join(HERE, "rvr1960.fingerprints"))
        fp4 = load_fingerprints(os.path.join(HERE, "rvr1960-4.fingerprints"))
        if not fp6:
            print("warning: no rvr1960.fingerprints; the RVR1960 check is skipped",
                  file=sys.stderr)
        r = scan(a.repo, refs, Rules(fp6, fp4, forbid))
    except GitError as e:
        print("cannot scan: %s" % e, file=sys.stderr)
        return 2
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump(r, f, indent=1)
    return 1 if report(r, forbid, a.quiet) else 0


if __name__ == "__main__":
    sys.exit(main())
