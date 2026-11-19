"""Round 40: the acceptance numbers from one try's logs (the analyst's scripts), short."""
import os, sys, csv
d = sys.argv[1]
def lines(name, keep):
    p = os.path.join(d, name + '.log')
    if not os.path.exists(p):
        return
    for l in open(p, encoding='utf-8', errors='replace'):
        if any(k in l for k in keep):
            print(f'[{name}] ' + l.rstrip()[:260])
lines('decay_beats', ['mf bass', 'mf middle', 'mf treble', 'ff bass'])
lines('bursts', ['mf middle', 'pp middle', 'ff treble', 'pp treble', 'mf treble', 'pp bass', 'mf bass'])
lines('octave_diff', ['middle_c    cand2', 'top         cand2', 'bass        cand2'])
lines('onset_shape', ['mf middle', 'mf bass', 'mf treble'])
lines('fill', ['A0 ', 'C1 ', 'D#1', 'F#1', "'cand2'"])
c = os.path.join(d, 'csv')
def rows(name):
    p = os.path.join(c, name)
    return list(csv.DictReader(open(p))) if os.path.exists(p) else []
for r in rows('treble_fundamental_decay.csv'):
    if r.get('dyn') == 'mf':
        print(f"[fund] {r['name']}: at3s {float(r['c2_at3s_re_peak']):.0f}/{float(r['sal_at3s_re_peak']):.0f}  late {float(r['c2_late_dbs']):.1f}/{float(r['sal_late_dbs']):.1f}  keyup {float(r['c2_after_keyup_0.5s_dbs']):.0f}/{float(r['sal_after_keyup_0.5s_dbs']):.0f}")
for r in rows('phrase_tails.csv'):
    if r.get('source') in ('cand2', 'Salamander'):
        print('[tails] ' + ' '.join(f'{k}={v}' for k, v in r.items() if k in ('phrase', 'source', 't_minus40_s', 'after_500ms', 'quietest_dbfs')))
R = rows('space_itd.csv')
for src in sorted(set(r['src'] for r in R)):
    ks = [r for r in R if r['src'] == src]
    lag = sum(abs(float(r['lag_ms'])) for r in ks) / max(1, len(ks))
    reg = {'bass': (21, 39), 'mid': (40, 83), 'top': (96, 108)}
    ild = {n: [float(r['ild_db']) for r in ks if lo <= int(r['key']) <= hi] for n, (lo, hi) in reg.items()}
    print(f"[itd] {src}: |ITD| {lag:.2f} ms; ILD bass {min(ild['bass']):+.0f}..{max(ild['bass']):+.0f}, top {min(ild['top']):+.0f}..{max(ild['top']):+.0f} dB")
for name in ('phrase_summary.csv',):
    for r in rows(name):
        if r.get('source') in ('cand2', 'Salamander'):
            print('[stereo] ' + ' '.join(f'{k}={v}' for k, v in r.items() if k in ('phrase', 'source', 'corr', 'lr_balance_db', 'side_minus_mid_db')))
p = os.path.join(d, 'metric.log')
if os.path.exists(p):
    for l in open(p):
        print('[metric] ' + l.rstrip())
