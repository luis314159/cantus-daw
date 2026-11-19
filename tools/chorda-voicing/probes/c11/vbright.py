"""Brightness against velocity, per register (round 40, candidate 11).
For each source dir's notes_pp / notes_mf / notes_ff (v 30 / 75 / 115; keys A0..C8 every minor
third, one every 6 s), the presence band's level (default 1.6-3.2 kHz) against the key's body
(f0*0.8 .. 1.2 kHz) over an early window (0.03-0.6 s after the onset) and a later one (0.6-2 s).
Prints, per register and dynamic, each source's (presence - body) minus the reference's (last dir).
   python -I vbright.py [--band lo,hi] [--keys] <dir1> ... <ref dir>"""
import sys, os
import numpy as np, soundfile as sf

args = sys.argv[1:]
lo, hi = 1600.0, 3200.0
perkey = False
while args and args[0].startswith('--'):
    a = args.pop(0)
    if a == '--band':
        lo, hi = map(float, args.pop(0).split(','))
    elif a == '--keys':
        perkey = True
dirs = args
KEYS = list(range(21, 109, 3))
DYN = ['pp', 'mf', 'ff']
WIN = [(0.03, 0.6), (0.6, 2.0)]
REG = [('bass A0-A2', 21, 45), ('tenor C3-A3', 48, 57), ('mid C4-A4', 60, 69), ('upper C5-C6', 72, 84)]


def measure(path):
    x, sr = sf.read(path, always_2d=True)
    m = x.sum(1)
    out = {}
    for i, k in enumerate(KEYS):
        if k > 84:
            continue
        s0 = int(i * 6.0 * sr)
        seg = m[s0:s0 + int(3.0 * sr)]
        a = np.abs(seg)
        on = int(np.argmax(a > 0.05 * a.max()))
        f0 = 440 * 2 ** ((k - 69) / 12)
        row = []
        for (t0, t1) in WIN:
            y = seg[on + int(t0 * sr):on + int(t1 * sr)]
            n = len(y)
            P = np.abs(np.fft.rfft(y * np.hanning(n))) ** 2
            F = np.fft.rfftfreq(n, 1 / sr)
            body = P[(F >= f0 * 0.8) & (F < max(1200, f0 * 2.5))].sum()
            pres = P[(F >= lo) & (F < hi)].sum()
            tot = P[(F >= 40) & (F < 16000)].sum()
            row.append((10 * np.log10(pres / body + 1e-30), 10 * np.log10(pres / tot + 1e-30)))
        out[k] = row
    return out


R = {}
for d in dirs:
    for dy in DYN:
        R[(d, dy)] = measure(os.path.join(d, f'notes_{dy}.wav'))
ref = dirs[-1]
name = lambda d: os.path.basename(os.path.normpath(d))[:10]
print(f'presence {lo:.0f}-{hi:.0f} Hz minus body, dB; sources minus {name(ref)} (ref: its absolute value)')
for w, (t0, t1) in enumerate(WIN):
    print(f'-- window {t0}-{t1} s')
    print(f'{"register":14} {"dyn":3} ' + ' '.join(f'{name(d):>10}' for d in dirs))
    for (rn, a, b) in REG:
        for dy in DYN:
            ks = [k for k in KEYS if a <= k <= b]
            vals = []
            for d in dirs:
                v = np.mean([R[(d, dy)][k][w][0] for k in ks])
                vals.append(v)
            r = vals[-1]
            print(f'{rn:14} {dy:3} ' + ' '.join(f'{v - r:+10.1f}' for v in vals[:-1]) + f' {r:10.1f}')
    # velocity slope: ff - pp and mf - pp of (presence - body)
    print(f'{"slope":14} ' + 'mf-pp/ff-mf per register: ' + '  '.join(
        name(d) + ' ' + ','.join(
            f'{np.mean([R[(d, "mf")][k][w][0] - R[(d, "pp")][k][w][0] for k in KEYS if a <= k <= b]):+.1f}/'
            f'{np.mean([R[(d, "ff")][k][w][0] - R[(d, "mf")][k][w][0] for k in KEYS if a <= k <= b]):+.1f}'
            for (rn, a, b) in REG) for d in dirs))
if perkey:
    print('-- per key, early window, source minus ref (pp mf ff)')
    for k in KEYS:
        if k > 84:
            continue
        print(f'{k:3} ' + '  '.join(name(d) + ' ' + ' '.join(
            f'{R[(d, dy)][k][0][0] - R[(ref, dy)][k][0][0]:+5.1f}' for dy in DYN) for d in dirs[:-1]))
