"""Read one listener's results file from a blind listening kit (the `abx_kit` page that
Chorda's `chorda_abx` and Tympanum's `tympanum_abx` make: the JSON a listener downloads and
sends, or one row the owner exports from the published page's summary). Prints the form, the
ABX scores (correct of answered, and the one-sided binomial p — the chance of that many or more
by guessing), each trial with its plays and changes, each preference with its note, and — per
item — the ABX answers and the preferences, which say *where* a source is told apart or
preferred. Which source a token is lives in the kit's key folder (`<kit>-key/key.json`): pass
it to name them.

    python -I abx_read.py <results.json> [<kit>-key/key.json]

Plain Python 3 (no third-party module). The files hold what a listener typed: keep them out
of the repository (they are personal data), as every round's results have been.
"""
import json
import sys
from math import comb


def p_at_least(k, n):
    return sum(comb(n, i) for i in range(k, n + 1)) / 2 ** n if n else float('nan')


def score(trials):
    done = [x for x in trials if x.get('answer')]
    return sum(1 for x in done if x['answer'] == x['x']), len(done)


def main():
    d = json.load(open(sys.argv[1], encoding='utf-8'))
    names = {}
    if len(sys.argv) > 2:
        key = json.load(open(sys.argv[2], encoding='utf-8'))
        if key.get('kit') and d.get('kit') and key['kit'] != d['kit']:
            print(f"warning: the results are of kit {d['kit']}, the key of {key['kit']}")
        for tok, who in (key.get('tokens') or {}).items():
            names[tok] = who if isinstance(who, str) else who.get('piano', str(who))
    nm = lambda t: names.get(t, t)
    print('kit', d.get('kit'), 'lang', d.get('lang'), 'started', d.get('started'),
          'main done', d.get('mainDone'), 'extra done', d.get('extraDone'), 'engine', d.get('engine'))
    print('listener', d.get('listener'), 'form', d.get('about'))
    print('browser', (d.get('ua') or '')[:120])
    for part in ('abx', 'extra'):
        k, n = score(d.get(part, []))
        if n:
            print(f'{part}: {k}/{n} correct, p = {p_at_least(k, n):.4f} (one-sided, by guessing)')
    for i, x in enumerate(d.get('abx', [])):
        print(f" abx {i + 1:2d} {x['phrase']:12s} A={nm(x['a'])} B={nm(x['b'])} X={nm(x['x'])} "
              f"answer={nm(x.get('answer'))} plays={x.get('plays')} heard={x.get('heard')} changes={x.get('changes')}")
    for i, x in enumerate(d.get('extra', [])):
        if x.get('answer'):
            print(f" extra {i + 1:2d} {x['phrase']:12s} X={nm(x['x'])} answer={nm(x['answer'])} plays={x.get('plays')}")
    for i, q in enumerate(d.get('pref', [])):
        print(f" pref {i + 1:2d} {q['phrase']:12s} {nm(q['one'])} / {nm(q['two'])} -> {nm(q.get('choice'))}"
              f" | {q.get('note', '')} | plays {q.get('plays')}")
    # Per item: the ABX right of answered (part 1, extra), and each preference as
    # "chosen > other" (or "=" for no preference); a catch question (one clip twice) as such.
    items = []
    for part in ('abx', 'extra', 'pref'):
        for x in d.get(part, []):
            if x['phrase'] not in items:
                items.append(x['phrase'])
    if not items:
        return
    print('\nper item (ABX right/answered; preferences: chosen > other, = none)')
    for it in items:
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
                prefs.append(f"catch({a}): {'passed' if q['choice'] == 'same' else 'failed'}")
            elif q['choice'] == 'same':
                prefs.append(f'{a} = {b}')
            else:
                win, lose = (a, b) if q['choice'] == 'one' else (b, a)
                prefs.append(f'{win} > {lose}')
        print(f" {it:12s} {cols[0]:9s} {cols[1]:11s} {'; '.join(prefs)}")


if __name__ == '__main__':
    main()
