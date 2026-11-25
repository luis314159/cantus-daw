"""Read listeners' results files from a blind listening kit (the `abx_kit` page that Chorda's
`chorda_abx`, Tympanum's `tympanum_abx` and the sound lab's `lab.py kit` make: the JSON a
listener downloads and sends, or one row the owner exports from the published page's summary).

    python -I abx_read.py <results.json> [<more results.json> ...] [<kit>-key/key.json]
                          [--alpha 0.05] [--power 0.8] [--quiet]

Per listener: the form; each ABX part's score with its one-sided binomial p (the chance of that
many or more by guessing) and its power; every trial (plays, changes) and preference (note);
the catch questions — one clip under two names, the expected answer "no preference" — with
each failure FLAGGED; the MUSHRA ratings per source (median, 95 % CI) and the post-screening of
ITU-R BS.1534-3 §4.1.2 (a listener who rates the hidden reference under 90 in more than 15 %
of the items is excluded; a low anchor rated at or over the hidden reference is flagged); a
per-item table. With two or more files, the pooled scores (everyone; only those who pass
every check), and the MUSHRA per source over the kept listeners with a sign test of each
source against the hidden reference.

**Power.** For an ABX of n trials at one-sided alpha, the critical count k is the least with
P(X >= k | guessing) <= alpha; the power against a listener who answers right with
probability p1 is P(X >= k | p1). The table gives, for p1 = 0.6, 0.7, 0.8, 0.9, the least
number of trials whose power reaches the stated level, and the power of the kit's own count.

Which source a token is lives in the kit's key folder (`<kit>-key/key.json`): pass it to name
them. Plain Python 3 (no third-party module). The files hold what a listener typed: keep them
out of the repository (they are personal data), as every round's results have been.
"""
import json
import random
import sys
from math import comb, sqrt

T975 = {1: 12.706, 2: 4.303, 3: 3.182, 4: 2.776, 5: 2.571, 6: 2.447, 7: 2.365, 8: 2.306, 9: 2.262,
        10: 2.228, 12: 2.179, 15: 2.131, 20: 2.086, 25: 2.060, 30: 2.042, 40: 2.021, 60: 2.000, 120: 1.980}


def p_at_least(k, n, p=0.5):
    """P(X >= k), X ~ Binomial(n, p)."""
    if not n:
        return float('nan')
    return sum(comb(n, i) * p ** i * (1 - p) ** (n - i) for i in range(max(k, 0), n + 1))


def critical(n, alpha):
    """The least k with P(X >= k | 0.5) <= alpha (n + 1 if none)."""
    for k in range(n + 1):
        if p_at_least(k, n) <= alpha:
            return k
    return n + 1


def power(n, p1, alpha):
    return p_at_least(critical(n, alpha), n, p1) if n else 0.0


def trials_for(p1, alpha, target, nmax=400):
    """The least n whose power against p1 reaches target (None within nmax)."""
    for n in range(1, nmax + 1):
        if power(n, p1, alpha) >= target:
            return n
    return None


def t975(df):
    if df <= 0:
        return float('nan')
    ks = sorted(T975)
    for k in ks:
        if df <= k:
            return T975[k]
    return 1.960


def median(v):
    v = sorted(v)
    n = len(v)
    return float('nan') if not n else (v[n // 2] if n % 2 else 0.5 * (v[n // 2 - 1] + v[n // 2]))


def mean_ci(v):
    """(mean, half-width of the 95 % CI by Student's t) — BS.1534-3 §9.2's statistic."""
    n = len(v)
    if n < 2:
        return (v[0] if v else float('nan')), float('nan')
    m = sum(v) / n
    s = sqrt(sum((x - m) ** 2 for x in v) / (n - 1))
    return m, t975(n - 1) * s / sqrt(n)


def median_ci(v, reps=4000, seed=1534):
    """The median's 95 % percentile-bootstrap CI (seeded: the same files read the same)."""
    if len(v) < 2:
        return float('nan'), float('nan')
    r = random.Random(seed)
    ms = sorted(median([r.choice(v) for _ in v]) for _ in range(reps))
    return ms[int(0.025 * reps)], ms[int(0.975 * reps) - 1]


def sign_test(pos, neg):
    """Two-sided sign test of pos against neg (ties dropped)."""
    n = pos + neg
    return min(1.0, 2 * p_at_least(max(pos, neg), n)) if n else float('nan')


def score(trials):
    done = [x for x in trials if x.get('answer')]
    return sum(1 for x in done if x['answer'] == x['x']), len(done)


class Key:
    def __init__(self, key):
        self.k = key or {}
        self.tokens = {}
        for tok, who in (self.k.get('tokens') or {}).items():
            self.tokens[tok] = who if isinstance(who, str) else who.get('piano', str(who))
        self.hidden = self.k.get('hidden_reference')

    def name(self, t):
        return self.tokens.get(t, t)


def catches(d, key):
    """[(item, passed)] of the catch questions answered: the two names one source."""
    out = []
    for q in d.get('pref', []):
        if q.get('choice') and q['one'] != q['two'] and key.name(q['one']) == key.name(q['two']) and key.tokens:
            out.append((q['phrase'], q['choice'] == 'same'))
    return out


def mushra(d, key):
    """{source: [scores]} and the post-screening: (hidden reference's scores, anchors' scores
    per item, the excluded verdict)."""
    by, items = {}, []
    for x in d.get('mushra', []) or []:
        row = {}
        for t in x.get('order', []):
            v = x.get('scores', {}).get(t)
            if v is None:
                continue
            src = 'hidden reference' if t == key.hidden else key.name(t)
            by.setdefault(src, []).append(v)
            row[src] = v
        if row:
            items.append((x['phrase'], row))
    hid = [r['hidden reference'] for _, r in items if 'hidden reference' in r]
    low = sum(v < 90 for v in hid)
    excluded = bool(hid) and low / len(hid) > 0.15
    anchors = sorted(s for s in by if s.startswith('anchor'))
    flags = []
    if anchors:
        lo = anchors[0]
        for it, r in items:
            if lo in r and 'hidden reference' in r and r[lo] >= r['hidden reference']:
                flags.append(f'{it}: {lo} {r[lo]} >= hidden reference {r["hidden reference"]}')
    return by, items, {'hidden_under_90': low, 'items': len(hid), 'excluded': excluded, 'anchor_flags': flags}


def show_listener(path, d, key, alpha, target, quiet):
    nm = key.name
    if key.k.get('kit') and d.get('kit') and key.k['kit'] != d['kit']:
        print(f"warning: {path} is of kit {d['kit']}, the key of {key.k['kit']}")
    print(f"\n== {path}")
    print('kit', d.get('kit'), 'lang', d.get('lang'), 'started', d.get('started'),
          'main done', d.get('mainDone'), 'extra done', d.get('extraDone'), 'engine', d.get('engine'))
    print('listener', d.get('listener'), 'form', d.get('about'))
    print('browser', (d.get('ua') or '')[:120])
    for part in ('abx', 'extra'):
        k, n = score(d.get(part, []))
        if n:
            print(f'{part}: {k}/{n} correct, p = {p_at_least(k, n):.4f} (one-sided, by guessing); '
                  f'critical {critical(n, alpha)}/{n} at alpha {alpha}; power '
                  + ', '.join(f'{power(n, p1, alpha):.2f} at p1 {p1}' for p1 in (0.6, 0.7, 0.8, 0.9)))
    c = catches(d, key)
    for it, ok in c:
        print(f'catch question ({it}): {"passed" if ok else "FAILED — FLAGGED: a preference between one clip and itself"}')
    if not key.tokens and any(d.get('pref', [])):
        print('catch questions: pass the key to find them')
    by, items, ps = mushra(d, key)
    if by:
        print('MUSHRA (0-100):')
        for src, v in sorted(by.items(), key=lambda kv: -median(kv[1])):
            lo, hi = median_ci(v)
            m, h = mean_ci(v)
            print(f'  {src:28s} median {median(v):5.1f} [{lo:.0f}, {hi:.0f}]  mean {m:5.1f} ± {h:.1f}  ({len(v)} items)')
        print(f"  post-screening: hidden reference under 90 in {ps['hidden_under_90']} of {ps['items']} items"
              + (' — EXCLUDED (BS.1534-3: more than 15 %)' if ps['excluded'] else ' — kept'))
        for f in ps['anchor_flags']:
            print(f'  FLAGGED anchor: {f}')
    if quiet:
        return c, ps
    for i, x in enumerate(d.get('abx', [])):
        print(f" abx {i + 1:2d} {x['phrase']:12s} A={nm(x['a'])} B={nm(x['b'])} X={nm(x['x'])} "
              f"answer={nm(x.get('answer'))} plays={x.get('plays')} heard={x.get('heard')} changes={x.get('changes')}")
    for i, x in enumerate(d.get('extra', [])):
        if x.get('answer'):
            print(f" extra {i + 1:2d} {x['phrase']:12s} X={nm(x['x'])} answer={nm(x['answer'])} plays={x.get('plays')}")
    for i, q in enumerate(d.get('pref', [])):
        print(f" pref {i + 1:2d} {q['phrase']:12s} {nm(q['one'])} / {nm(q['two'])} -> {nm(q.get('choice'))}"
              f" | {q.get('note', '')} | plays {q.get('plays')}")
    for it, row in items:
        print(f" mushra {it:12s} " + ', '.join(f'{s} {v}' for s, v in sorted(row.items())))
    # Per item: the ABX right of answered, each preference as "chosen > other" (or "=").
    its = []
    for part in ('abx', 'extra', 'pref'):
        for x in d.get(part, []):
            if x['phrase'] not in its:
                its.append(x['phrase'])
    if its:
        print('\nper item (ABX right/answered; preferences: chosen > other, = none)')
    for it in its:
        cols = []
        for part in ('abx', 'extra'):
            k, n = score([x for x in d.get(part, []) if x['phrase'] == it])
            cols.append(f'{part} {k}/{n}' if n else f'{part} -')
        prefs = []
        for q in d.get('pref', []):
            if q['phrase'] != it or not q.get('choice'):
                continue
            a, b = nm(q['one']), nm(q['two'])
            if a == b:
                prefs.append(f"catch({a}): {'passed' if q['choice'] == 'same' else 'FAILED'}")
            elif q['choice'] == 'same':
                prefs.append(f'{a} = {b}')
            else:
                win, lose = (a, b) if q['choice'] == 'one' else (b, a)
                prefs.append(f'{win} > {lose}')
        print(f" {it:12s} {cols[0]:9s} {cols[1]:11s} {'; '.join(prefs)}")
    return c, ps


def main(argv):
    try:
        sys.stdout.reconfigure(encoding='utf-8')
    except (AttributeError, ValueError):
        pass
    alpha, target, quiet = 0.05, 0.8, False
    args = list(argv)
    if '--alpha' in args:
        i = args.index('--alpha'); alpha = float(args[i + 1]); del args[i:i + 2]
    if '--power' in args:
        i = args.index('--power'); target = float(args[i + 1]); del args[i:i + 2]
    if '--quiet' in args:
        args.remove('--quiet'); quiet = True
    if not args:
        print(__doc__)
        return 2
    key, results = None, []
    for p in args:
        d = json.load(open(p, encoding='utf-8'))
        if 'tokens' in d and 'abx' not in d:
            key = d
        elif 'listeners' in d:  # the owner's "download everything"
            key = key or {'tokens': d.get('tokens'), 'kit': d.get('kit')}
            results += [(f"{p}#{x.get('listener')}", x.get('results') or {}) for x in d['listeners']]
        else:
            results.append((p, d))
    key = Key(key)
    print(f'power at alpha {alpha} (one-sided): trials for {target:.0%} power — '
          + ', '.join(f'p1 {p1}: {trials_for(p1, alpha, target)}' for p1 in (0.6, 0.7, 0.8, 0.9)))
    seen = []
    for p, d in results:
        c, ps = show_listener(p, d, key, alpha, target, quiet)
        seen.append((p, d, c, ps))
    if len(seen) < 2:
        return 0
    print('\n== pooled')
    for label, keep in (('everyone', lambda c, ps: True),
                        ('passing every check', lambda c, ps: all(ok for _, ok in c) and not ps['excluded'])):
        rows = [(p, d) for p, d, c, ps in seen if keep(c, ps)]
        for part in ('abx', 'extra'):
            K = N = sig = full = 0
            for _, d in rows:
                k, n = score(d.get(part, []))
                if n:
                    K, N, full = K + k, N + n, full + 1
                    sig += p_at_least(k, n) < alpha
            if N:
                print(f'{label:20s} {part}: {K}/{N}, p = {p_at_least(K, N):.2e}; {sig} of {full} listeners significant alone')
    kept = [d for p, d, c, ps in seen if not ps['excluded']]
    pooled = {}
    pairs = {}
    for d in kept:
        by, items, _ = mushra(d, key)
        for s, v in by.items():
            pooled.setdefault(s, []).extend(v)
        for _, r in items:
            h = r.get('hidden reference')
            if h is None:
                continue
            for s, v in r.items():
                if s != 'hidden reference':
                    pp = pairs.setdefault(s, [0, 0])
                    pp[0] += v > h
                    pp[1] += v < h
    if pooled:
        print(f'MUSHRA over {len(kept)} listener(s) kept by the post-screening:')
        for s, v in sorted(pooled.items(), key=lambda kv: -median(kv[1])):
            lo, hi = median_ci(v)
            m, h = mean_ci(v)
            st = pairs.get(s)
            vs = f'; against the hidden reference {st[0]} above, {st[1]} below, sign test p = {sign_test(*st):.3g}' if st else ''
            print(f'  {s:28s} median {median(v):5.1f} [{lo:.0f}, {hi:.0f}]  mean {m:5.1f} ± {h:.1f}  ({len(v)}){vs}')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
