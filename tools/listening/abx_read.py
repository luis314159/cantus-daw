"""Read one listener's results file from Chorda's blind test (`chorda_abx`'s page: the JSON a
listener downloads and sends, or one row the owner exports from the published page's summary).
Prints the form, the ABX score (correct of answered, and the one-sided binomial p — the chance
of that many or more by guessing), each trial with its plays and changes, and each preference
with its note. Which piano a token is lives in the kit's key folder (`<kit>-key/key.json`):
pass it to name them.

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
        for tok, who in (key.get('tokens') or {}).items():
            names[tok] = who if isinstance(who, str) else who.get('piano', str(who))
    nm = lambda t: names.get(t, t)
    print('kit', d.get('kit'), 'lang', d.get('lang'), 'started', d.get('started'),
          'main done', d.get('mainDone'), 'extra done', d.get('extraDone'), 'engine', d.get('engine'))
    print('form', d.get('about'))
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


if __name__ == '__main__':
    main()
