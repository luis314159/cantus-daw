"""Round 40: the analyst's headline numbers for two tries side by side, with the Salamander's.

    python compare.py <out dir A> <out dir B>
"""
import csv, os, sys
import numpy as np

REG = {'bass': (21, 47), 'middle': (48, 83), 'treble': (84, 108)}


def rows(d, name):
    p = os.path.join(d, 'csv', name)
    return list(csv.DictReader(open(p))) if os.path.exists(p) else []


def f(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return float('nan')


def med(v):
    v = [x for x in v if np.isfinite(x)]
    return float(np.median(v)) if v else float('nan')


def numbers(d):
    out = {}
    R = rows(d, 'decay_beats.csv')
    for reg, (lo, hi) in REG.items():
        for src, tag in (('c2', 'c'), ('sal', 's')):
            g = [r for r in R if r['src'] == src and r['dyn'] == 'mf' and lo <= int(r['key']) <= hi]
            out[f'{tag} {reg} beat nulls dB'] = med([f(r['null_depth_db']) for r in g])
            out[f'{tag} {reg} partials 0.05-0.5 s dB/s'] = med([f(r['partial_prompt_dbs']) for r in g])
            out[f'{tag} {reg} partials 1.5-3.4 s dB/s'] = med([f(r['partial_late_dbs']) for r in g])
            out[f'{tag} {reg} drop in 1st s dB'] = med([f(r['drop_first_1s_db']) for r in g])
    T = {r['name']: r for r in rows(d, 'treble_fundamental_decay.csv') if r['dyn'] == 'mf'}
    for n in ('C6', 'F#6', 'A6', 'C7', 'D#7', 'F#7'):
        if n in T:
            out[f'c {n} at 3 s dB'] = f(T[n]['c2_at3s_re_peak'])
            out[f's {n} at 3 s dB'] = f(T[n]['sal_at3s_re_peak'])
    I = rows(d, 'space_itd.csv')
    for src, tag in (('c2', 'c'), ('sal', 's')):
        ks = [r for r in I if r['src'] == src]
        if ks:
            out[f'{tag} |ITD| ms'] = float(np.mean([abs(f(r['lag_ms'])) for r in ks]))
            out[f'{tag} ILD A0-D#2 dB'] = float(np.mean([f(r['ild_db']) for r in ks if int(r['key']) <= 39]))
            out[f'{tag} ILD C7-C8 dB'] = float(np.mean([f(r['ild_db']) for r in ks if int(r['key']) >= 96]))
    for r in rows(d, 'phrase_summary.csv'):
        tag = {'cand2': 'c', 'Salamander': 's'}.get(r['source'])
        if tag and r['phrase'] in ('top', 'fast_run', 'ballad', 'bass'):
            out[f'{tag} corr {r["phrase"]}'] = f(r['corr'])
    for r in rows(d, 'space_early.csv'):
        tag = {'cand2': 'c', 'Salamander': 's'}.get(r['source'])
        if tag and r['phrase'] == 'top':
            out[f'{tag} top: early 5-80 ms re direct dB'] = f(r['early_5_80ms_vs_direct_db'])
    for r in rows(d, 'phrase_tails.csv'):
        tag = {'cand2': 'c', 'Salamander': 's'}.get(r['source'])
        if tag and r['phrase'] in ('ff_chord', 'ballad', 'bass', 'middle_c'):
            out[f'{tag} {r["phrase"]}: to -40 dB after release s'] = f(r['t_minus40_s'])
    B = rows(d, 'notes_bursts.csv')
    for dyn in ('pp', 'mf'):
        for src, tag in (('c2', 'c'), ('sal', 's')):
            out[f'{tag} middle {dyn}: release spike dB'] = med([f(r['release_spike_db']) for r in B if r['src'] == src and r['dyn'] == dyn and 48 <= int(r['key']) <= 83])
    for r in rows(d, 'phrase_octave_diff.csv'):
        if r['source'] == 'cand2' and r['phrase'] == 'middle_c':
            for c in ('1000', '4000', '8000'):
                out[f'c middle_c octave {c} Hz vs Salamander dB'] = f(r[c])
                out[f's middle_c octave {c} Hz vs Salamander dB'] = 0.0
    On = rows(d, 'onset_shape.csv')
    for src, tag in (('c2', 'c'), ('sal', 's')):
        out[f'{tag} middle mf: rise -50 to -20 dB ms'] = med([f(r['lead_in_ms']) for r in On if r['src'] == src and r['dyn'] == 'mf' and 48 <= int(r['key']) <= 83])
    return out


def main():
    a, b = numbers(sys.argv[1]), numbers(sys.argv[2])
    keys = [k[2:] for k in a if k.startswith('c ')]
    print(f'{"":46} {"before":>8} {"after":>8} {"Salamander":>10}')
    for k in keys:
        print(f'{k:46} {a.get("c " + k, float("nan")):8.2f} {b.get("c " + k, float("nan")):8.2f} {a.get("s " + k, float("nan")):10.2f}')


if __name__ == '__main__':
    main()
