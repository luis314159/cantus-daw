"""Fold a profile.ps1 sample file into a readable report.

  python profile_report.py cantus.samples [--from 2000] [--to 45000] [--bucket 1000] [--top 25]
                                          [--focus substring]

Prints: samples per thread per time bucket; then, for the window asked, each busy thread's
top functions by *self* samples and by *inclusive* samples (a function counted once per
sample however often it recurs). Frames from the system (ntdll, kernel32, …) count as self
time of the first frame of the program found above them when --app-self is given.
"""

import argparse
import collections
import sys


WAITS = ("ZwWait", "NtWait", "ZwDelayExecution", "NtDelayExecution", "ZwRemoveIoCompletion",
         "NtRemoveIoCompletion", "ZwUserMsgWaitForMultipleObjectsEx", "NtUserMsgWaitForMultipleObjectsEx",
         "ZwAlertThreadByThreadId", "NtWaitForAlertByThreadId", "ZwYieldExecution", "NtUserGetMessage",
         "ZwUserGetMessage")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("file")
    ap.add_argument("--from", dest="t0", type=int, default=0)
    ap.add_argument("--to", dest="t1", type=int, default=10**9)
    ap.add_argument("--bucket", type=int, default=1000)
    ap.add_argument("--top", type=int, default=25)
    ap.add_argument("--focus", default="")
    ap.add_argument("--thread", default="")
    ap.add_argument("--app-self", action="store_true")
    ap.add_argument("--prefix", default="cantus")
    a = ap.parse_args()

    rows = []
    cpu = collections.defaultdict(list)  # thread -> [(ms, cpu in 100 ns)]
    with open(a.file, encoding="utf-8", errors="replace") as f:
        for line in f:
            parts = line.rstrip("\n").split("\t")
            if parts[0] == "cpu" and len(parts) == 5:
                cpu[f"{parts[3]}#{parts[2]}"].append((int(parts[1]), int(parts[4])))
                continue
            if len(parts) < 4:
                continue
            t, tid, name, stack = int(parts[0]), parts[1], parts[2], parts[3]
            frames = [x for x in stack.split(";") if x]
            # A thread caught waiting used its CPU before the look, not in it.
            if frames and any(frames[0].startswith(w) for w in WAITS):
                continue
            rows.append((t, f"{name}#{tid}", frames))

    # Exact CPU per thread per bucket, from the thread times.
    if cpu:
        per = collections.defaultdict(collections.Counter)
        totals = collections.Counter()
        for th, pts in cpu.items():
            pts.sort()
            for (t0, c0), (t1, c1) in zip(pts, pts[1:]):
                ms = (c1 - c0) / 10_000
                per[t0 // a.bucket][th] += ms
                if a.t0 <= t0 < a.t1:
                    totals[th] += ms
        top = [th for th, _ in totals.most_common(10)]
        print(f"CPU ms per {a.bucket} ms (exact, thread times):")
        print("t(s)   total " + " ".join(f"{th[:16]:>16}" for th in top))
        for b in sorted(per):
            tot = sum(per[b].values())
            print(f"{b * a.bucket / 1000:5.1f} {tot:6.0f} " + " ".join(f"{per[b][th]:>16.0f}" for th in top))
        span = (a.t1 if a.t1 < 10**9 else max(t for pts in cpu.values() for t, _ in pts)) - a.t0
        print(f"in [{a.t0}, {a.t0 + span}) ms: {sum(totals.values()) / span * 100:.1f} % of a core")
        for th, ms in totals.most_common(15):
            print(f"  {ms / span * 100:6.2f} %  {th}")
        print()

    # Per-thread timeline.
    buckets = collections.defaultdict(collections.Counter)
    per_thread = collections.Counter()
    for t, th, _ in rows:
        buckets[t // a.bucket][th] += 1
        per_thread[th] += 1
    threads = [th for th, _ in per_thread.most_common(12)]
    print("samples (running) per", a.bucket, "ms:")
    print("t(s)  " + " ".join(f"{th[:18]:>18}" for th in threads))
    for b in sorted(buckets):
        print(f"{b * a.bucket / 1000:5.1f} " + " ".join(f"{buckets[b][th]:>18}" for th in threads))

    sel = [r for r in rows if a.t0 <= r[0] < a.t1 and (not a.thread or a.thread in r[1])]
    if a.focus:
        sel = [r for r in sel if any(a.focus in fr for fr in r[2])]
    by_thread = collections.defaultdict(list)
    for r in sel:
        by_thread[r[1]].append(r[2])
    for th, stacks in sorted(by_thread.items(), key=lambda kv: -len(kv[1])):
        if len(stacks) < 5:
            continue
        print(f"\n== {th}: {len(stacks)} samples in [{a.t0}, {a.t1}) ms")
        selfc = collections.Counter()
        incl = collections.Counter()
        for fr in stacks:
            if not fr:
                continue
            s = fr[0]
            if a.app_self:
                for x in fr:
                    if a.prefix in x:
                        s = x
                        break
            selfc[s] += 1
            for x in set(fr):
                incl[x] += 1
        print("  self:")
        for n, c in selfc.most_common(a.top):
            print(f"    {c:6d} {100 * c / len(stacks):5.1f}%  {n[:150]}")
        print("  inclusive (program frames):")
        k = 0
        for n, c in incl.most_common():
            if a.prefix not in n:
                continue
            print(f"    {c:6d} {100 * c / len(stacks):5.1f}%  {n[:150]}")
            k += 1
            if k >= a.top:
                break


if __name__ == "__main__":
    sys.exit(main())
