"""The human-readable report of tools/public/scan.py. Forbid-list entries are never echoed
whole (the list is private): a finding names the entry by number and its first letters."""


def _where(f):
    for k in ("paths", "commit_message", "tag_message"):
        if k in f:
            v = f[k]
            return ", ".join(v) if isinstance(v, list) else "%s %s" % (k.replace("_", " "), v)
    if "identity" in f:
        return "%s of %s" % (f["identity"], f["where"])
    if "path" in f:
        return "path %s (commit %s)" % (f["path"], f["commit"][:10])
    return "?"


def report(r, forbid, quiet=False):
    out = []
    say = out.append
    say("Repository %s, refs %s: %d commits, %d distinct blobs (%s), %d annotated tags"
        % (r["repo"], " ".join(r["refs"]), r["commits"], r["blobs"],
           ", ".join("%d %s" % (n, k) for k, n in sorted(r["blob_kinds"].items())),
           r["annotated_tags"]))
    fails = 0

    def section(title, items, fail):
        nonlocal fails
        mark = ("FAIL" if fail else "note") if items else "ok"
        fails += bool(items) and fail
        say("\n[%s] %s: %d" % (mark, title, len(items)))
        return bool(items) and not (quiet and not fail)

    fp = r["forbidden_paths"]
    if section("forbidden paths in history", fp, True):
        for p, v in list(fp.items())[:40]:
            say("  %s  [%s] (%d commits, newest %s)" % (p, v["rule"], v["commits"],
                                                       v["newest"][:10]))
        if len(fp) > 40:
            say("  ... and %d more" % (len(fp) - 40))
    if section("blobs with RVR1960 text (6-word)", r["rvr1960"], True):
        for f in sorted(r["rvr1960"], key=lambda f: -f["hits"]):
            say("  %4d shingles  %s  (blob %s, in %d commits, newest %s)"
                % (f["hits"], ", ".join(f["paths"]), f["blob"][:10], f["commits"],
                   f["newest_commit"][:10]))
    if section("secrets (high confidence)", r["secrets"], True):
        for f in r["secrets"]:
            say("  %s: %s  %s" % (f["rule"], f["match"], _where(f)))
    if section("secret-shaped files", r["secret_files"], True):
        for p, v in r["secret_files"].items():
            say("  %s  (%d commits, newest %s)" % (p, v["commits"], v["newest"][:10]))
    title = "forbid-list hits (%d entries)" % r["forbid_entries"]
    if section(title, r["forbid"], True):
        seen = set()
        for f in r["forbid"]:
            line = "  entry %d (%s...)  %s" % (f["entry"] + 1, forbid[f["entry"]][:3], _where(f))
            if line not in seen:
                seen.add(line)
                say(line)
            if len(seen) >= 60:
                say("  ... and more")
                break
    if section("blobs with RVR1960 4-word runs (review)", r["rvr1960_4"], False):
        for f in sorted(r["rvr1960_4"], key=lambda f: -f["hits"])[:40]:
            say("  %4d  %s  (blob %s)" % (f["hits"], ", ".join(f["paths"]), f["blob"][:10]))
    if section("to review (low confidence)", r["review"], False):
        for f in r["review"][:60]:
            say("  %s: %s  %s" % (f["rule"], f["match"], _where(f)))
    imgs = r["images_outside_own_folders"]
    if section("images/PDFs outside our own picture folders (review)", imgs, False):
        for p, c in imgs.items():
            say("  %s  (commit %s)" % (p, c[:10]))
    if section("commit and tag identities", r["identities"], False):
        for k, v in r["identities"].items():
            say("  %5d  %s" % (v, k))
    if section("e-mail addresses in files", r["emails_in_files"], False):
        for e, ps in r["emails_in_files"].items():
            say("  %s  in %s" % (e, ", ".join(ps[:4]) + (" ..." if len(ps) > 4 else "")))
    if section("local user paths in files", r["user_paths_in_files"], False):
        for u, ps in r["user_paths_in_files"].items():
            say("  %s  in %s" % (u, ", ".join(ps[:4]) + (" ..." if len(ps) > 4 else "")))
    say("\n%s" % ("CLEAN: nothing that fails." if not fails
                  else "NOT READY: %d failing check(s)." % fails))
    print("\n".join(out))
    return fails
