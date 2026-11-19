"""Round 40: one number for "how close to the Salamander", built on the independent analyst's
measurements (its scripts' CSVs for one try), each difference divided by what the analyst
named as near enough - so 1 means "at the edge of the target" for every term. Printed by
group (P1 ... P7, the report's findings) and in total.

    python accept.py <out dir>
"""
import csv, os, sys
import numpy as np


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


REG = {'bass': (21, 47), 'middle': (48, 83), 'treble': (84, 108)}


def score(d, verbose=False):
    parts = {}
    # P1: two-stage decay and beats, per register (mf and ff).
    R = rows(d, 'decay_beats.csv')
    p1 = []
    for dyn in ('mf', 'ff'):
        for reg, (lo, hi) in REG.items():
            g = {s: [r for r in R if r['src'] == s and r['dyn'] == dyn and lo <= int(r['key']) <= hi] for s in ('sal', 'c2')}
            m = {s: {c: med([f(r[c]) for r in g[s]]) for c in ('drop_first_1s_db', 'null_depth_db', 'partial_prompt_dbs', 'partial_late_dbs')} for s in g}
            p1.append(abs(m['c2']['drop_first_1s_db'] - m['sal']['drop_first_1s_db']) / 3)
            p1.append(max(0.0, m['c2']['null_depth_db'] - m['sal']['null_depth_db']) / 3)
            p1.append(abs(m['c2']['partial_prompt_dbs'] - m['sal']['partial_prompt_dbs']) / 6)
            if reg != 'treble':  # the treble's late part is measured on its fundamental (P2)
                p1.append(abs(m['c2']['partial_late_dbs'] - m['sal']['partial_late_dbs']) / 2)
    parts['P1 beats, two stages'] = float(np.nanmean(p1))
    # P2: the treble lives - its fundamental's level at 3 s and its late decay.
    T = [r for r in rows(d, 'treble_fundamental_decay.csv') if r['dyn'] == 'mf' and int(r['key']) >= 84]
    p2 = [abs(f(r['c2_at3s_re_peak']) - f(r['sal_at3s_re_peak'])) / 10 for r in T]
    p2 += [abs(f(r['c2_late_dbs']) - f(r['sal_late_dbs'])) / 4 for r in T]
    parts['P2 treble'] = float(np.nanmean(p2))
    # P3: stereo - arrival-time difference, level difference, correlation per phrase.
    I = rows(d, 'space_itd.csv')
    p3 = []
    if I:
        lag = {s: np.mean([abs(f(r['lag_ms'])) for r in I if r['src'] == s]) for s in ('sal', 'c2')}
        p3.append(abs(lag['c2'] - lag['sal']) / 0.15)
        for lo, hi in ((21, 39), (96, 108), (40, 95)):
            ild = {s: np.mean([f(r['ild_db']) for r in I if r['src'] == s and lo <= int(r['key']) <= hi]) for s in ('sal', 'c2')}
            p3.append(abs(ild['c2'] - ild['sal']) / 2.5)
    S = rows(d, 'phrase_summary.csv')
    for ph in sorted(set(r['phrase'] for r in S)):
        v = {r['source']: r for r in S if r['phrase'] == ph}
        if 'cand2' in v and 'Salamander' in v:
            p3.append(abs(f(v['cand2']['corr']) - f(v['Salamander']['corr'])) / 0.1)
            p3.append(abs(f(v['cand2']['lr_balance_db']) - f(v['Salamander']['lr_balance_db'])) / 2.5)
    parts['P3 stereo'] = float(np.nanmean(p3)) if p3 else float('nan')
    # P4: space - the treble's early energy after a staccato and its side share.
    E = {(r['phrase'], r['source']): r for r in rows(d, 'space_early.csv')}
    p4 = []
    for ph in ('top', 'fast_run'):
        a, b = E.get((ph, 'cand2')), E.get((ph, 'Salamander'))
        if a and b:
            p4.append(abs(f(a['early_5_80ms_vs_direct_db']) - f(b['early_5_80ms_vs_direct_db'])) / 4)
            p4.append(abs(f(a['late_80_195ms_vs_direct_db']) - f(b['late_80_195ms_vs_direct_db'])) / 4)
            p4.append(max(0.0, f(a['side_vs_mid_5_80ms_db']) - f(b['side_vs_mid_5_80ms_db'])) / 2)
    parts['P4 space'] = float(np.nanmean(p4)) if p4 else float('nan')
    # P5: endings - time to -40 dB after the last release or the pedal, the release spike.
    Tl = {(r['phrase'], r['source']): r for r in rows(d, 'phrase_tails.csv')}
    p5 = []
    for ph in ('ff_chord', 'ballad', 'bass', 'soft_chords', 'middle_c', 'fast_run'):
        a, b = Tl.get((ph, 'cand2')), Tl.get((ph, 'Salamander'))
        if a and b:
            p5.append(abs(f(a['t_minus40_s']) - f(b['t_minus40_s'])) / 0.2)
            p5.append(abs(f(a['after_500ms']) - f(b['after_500ms'])) / 5)
    B = rows(d, 'notes_bursts.csv')
    for dyn in ('pp', 'mf'):
        for reg, (lo, hi) in REG.items():
            v = {s: med([f(r['release_spike_db']) for r in B if r['src'] == s and r['dyn'] == dyn and lo <= int(r['key']) <= hi]) for s in ('sal', 'c2')}
            p5.append(max(0.0, v['c2'] - v['sal'] - 1) / 3)
    parts['P5 endings'] = float(np.nanmean(p5)) if p5 else float('nan')
    # P6: colour - octave differences at equal loudness (63 Hz ... 8 kHz), every phrase.
    O = rows(d, 'phrase_octave_diff.csv')
    p6 = []
    for r in O:
        if r['source'] == 'cand2':
            v = np.array([f(r[c]) for c in ('63', '125', '250', '500', '1000', '2000', '4000', '8000')])
            w = 1.5 if r['phrase'] in ('middle_c', 'bass') else 1.0
            p6.append(w * float(np.sqrt(np.nanmean(v ** 2))) / 3)
    parts['P6 colour'] = float(np.nanmean(p6)) if p6 else float('nan')
    # P7: the attack - the first 4 ms against the peak of 10-30 ms (the "click"). The
    # reference's -50 -> -20 dB lead-in is partly its samples' recorded pre-attack (the
    # analyst's caveat), not a target to copy.
    On = rows(d, 'onset_shape.csv')
    p7 = []
    for dyn in ('mf', 'ff'):
        for reg, (lo, hi) in REG.items():
            v = {s: med([f(r['spike_db']) for r in On if r['src'] == s and r['dyn'] == dyn and lo <= int(r['key']) <= hi]) for s in ('sal', 'c2')}
            p7.append(abs(v['c2'] - v['sal']) / 2)
    # The hammer's flash: the attack's centroid (0-50 ms) against the sustain's (60-310 ms).
    F = rows(d, 'notes_features.csv')
    for dyn in ('mf', 'ff'):
        for reg, (lo, hi) in REG.items():
            v = {s: med([np.log2(f(r['centroid_att']) / f(r['centroid_early'])) for r in F
                         if r['src'] == s and r['dyn'] == dyn and lo <= int(r['key']) <= hi
                         and f(r['centroid_early']) > 0]) for s in ('sal', 'c2')}
            p7.append(abs(v['c2'] - v['sal']) / 0.3)
    parts['P7 attack'] = float(np.nanmean(p7)) if p7 else float('nan')
    # P8: dynamics - mf against pp (LU of the first second) per register.
    p8 = []
    for reg, (lo, hi) in REG.items():
        dv = {}
        for s in ('sal', 'c2'):
            lv = {dyn: {int(r['key']): f(r['lufs1s']) for r in F if r['src'] == s and r['dyn'] == dyn and lo <= int(r['key']) <= hi} for dyn in ('pp', 'mf')}
            dv[s] = med([lv['mf'][k] - lv['pp'][k] for k in lv['mf'] if k in lv['pp']])
        p8.append(abs(dv['c2'] - dv['sal']) / 2)
    parts['P8 dynamics'] = float(np.nanmean(p8)) if p8 else float('nan')
    if any(not np.isfinite(v) for v in parts.values()):
        # A measurement that failed is no measurement: never a better score.
        return 1.0e9, parts
    total = float(np.nansum([1.5 * parts['P1 beats, two stages'], 1.2 * parts['P2 treble'], parts['P3 stereo'],
                             0.7 * parts['P4 space'], parts['P5 endings'], parts['P6 colour'], 0.4 * parts['P7 attack'],
                             0.5 * parts['P8 dynamics']]))
    if verbose:
        for k, v in parts.items():
            print(f'{k:24} {v:6.2f}')
        print(f'ACCEPT {total:.3f}')
    return total, parts


if __name__ == '__main__':
    score(sys.argv[1], True)
