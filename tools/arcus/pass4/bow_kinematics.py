import sys, numpy as np, soundfile as sf
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sound-refs")); import cantus_paths as cp  # noqa: E402
d = str(cp.refs("study", "zenodo-pamies-vila-bow-kinematics", "data"))  # $CANTUS_REFS (tools/sound-refs)
which = sys.argv[1] if len(sys.argv) > 1 else "Player"
M = np.genfromtxt(d + rf"\DataMocap_{which}.csv", delimiter=",", skip_header=1)
t = M[:, 0]; fs = 240.0
tip, frog = M[:, 1:4], M[:, 4:7]
nut, bridge = M[:, 16:19], M[:, 19:22]
ax = tip - frog; L = np.linalg.norm(ax, axis=1); u = ax / L[:, None]
print("bow length (m) median", np.nanmedian(L))
# bow displacement along its own axis: frog's position projected on the bow axis, relative to the bridge
pos = np.einsum("ij,ij->i", frog - bridge, u)
from scipy.signal import savgol_filter
ok = np.isfinite(pos)
pos = np.interp(t, t[ok], pos[ok])
v = savgol_filter(pos, 13, 3, deriv=1, delta=1 / fs)
a = savgol_filter(pos, 13, 3, deriv=2, delta=1 / fs)
# bow-bridge distance: the bridge marker to the bow line (a proxy; the contact's own changes)
w = bridge - frog
dist = np.linalg.norm(w - np.einsum("ij,ij->i", w, u)[:, None] * u, axis=1)
x, sr = sf.read(d + rf"\sound_{'player' if which=='Player' else 'case'+which[-1]}.wav")
if x.ndim > 1: x = x.mean(1)
hop = int(sr / fs)
n = len(x) // hop
e = np.array([np.sqrt(np.mean(x[i*hop:(i+1)*hop] ** 2)) + 1e-9 for i in range(n)])
edb = 20 * np.log10(e)
print("speed |v| p50 %.3f p90 %.3f max %.3f m/s" % (np.median(abs(v)), np.percentile(abs(v), 90), abs(v).max()))
# strokes: sign changes of v with |v| > 5 cm/s on both sides
s = np.sign(v)
rev = np.nonzero((s[1:] != s[:-1]))[0]
strokes = []
for i0, i1 in zip(rev[:-1], rev[1:]):
    seg = abs(v[i0:i1])
    if i1 - i0 < 0.08 * fs or seg.max() < 0.05: continue
    pk = seg.max(); ipk = int(np.argmax(seg))
    # the stroke's plateau: median speed over the middle half
    mid = np.median(seg[len(seg)//4: 3*len(seg)//4])
    t50 = np.argmax(seg > 0.5 * mid) / fs
    t90 = np.argmax(seg > 0.9 * mid) / fs
    amax = np.max(abs(a[i0:i0 + max(ipk, 1) + 1]))
    # the level: the audio's rise after the reversal (dB re the stroke's median)
    k0, k1 = i0, min(i1, n - 1)
    lev = edb[k0:k1]
    med = np.median(lev) if len(lev) else np.nan
    r3 = np.argmax(lev > med - 3) / fs if len(lev) else np.nan
    strokes.append((t[i0], (i1 - i0) / fs, mid, pk, t50, t90, amax, np.median(dist[i0:i1]), r3, edb[k0-1] - med if k0 > 0 else np.nan))
S = np.array(strokes)
print("strokes", len(S))
print(" dur s p10/50/90", np.percentile(S[:, 1], [10, 50, 90]).round(3))
print(" plateau speed m/s", np.percentile(S[:, 2], [10, 50, 90]).round(3))
print(" t to 50% of plateau s", np.percentile(S[:, 4], [10, 50, 90]).round(3))
print(" t to 90% of plateau s", np.percentile(S[:, 5], [10, 50, 90]).round(3))
print(" peak accel m/s2", np.percentile(S[:, 6], [10, 50, 90]).round(2))
print(" bow-bridge proxy m", np.percentile(S[:, 7], [10, 50, 90]).round(4))
print(" audio rise to -3 dB after reversal s", np.nanpercentile(S[:, 8], [10, 50, 90]).round(3))
print(" level at the reversal re stroke dB", np.nanpercentile(S[:, 9], [10, 50, 90]).round(1))
# starts from silence: strokes whose preceding 150 ms are 25 dB under their median
q = []
for st in strokes:
    i0 = int(st[0] * fs)
    if i0 > 40 and np.max(edb[i0-36:i0-2]) < np.median(edb[i0:i0+int(st[1]*fs)]) - 20:
        q.append(st)
print("starts from silence:", len(q))
for st in q:
    i0 = int(st[0] * fs)
    seg = abs(v[i0:i0 + 60]); lv = edb[i0:i0+60] - np.median(edb[i0:i0+int(st[1]*fs)])
    print("  t %.2f dur %.2f plateau %.3f t90 %.3f amax %.2f | v every 25 ms:" % (st[0], st[1], st[2], st[5], st[6]), (seg[::6]).round(3), "| dB:", lv[::6].round(1))
print("first strokes:")
for st in strokes[:6]:
    i0 = int(st[0] * fs)
    seg = abs(v[i0:i0 + 72]); lv = edb[i0:i0+72] - np.median(edb[i0:i0+int(st[1]*fs)])
    print("  t %.2f dur %.2f plateau %.3f t50 %.3f t90 %.3f amax %.2f\n   v/25ms:" % (st[0], st[1], st[2], st[4], st[5], st[6]), (seg[::6]).round(3), "\n   dB/25ms:", lv[::6].round(1))
# the audio before the first stroke
i0 = int(strokes[0][0] * fs)
print("level 0.5 s before the first stroke re its median:", (edb[max(i0-120,0):i0:12] - np.median(edb[i0:i0+100])).round(1))
# the level's rise against the speed's within strokes: regress dB on 20log10(v) over the first 0.3 s of each stroke
X=[];Y=[]
for st in strokes:
    i0 = int(st[0] * fs); n1 = min(int(0.3*fs), int(st[1]*fs))
    vv = abs(v[i0:i0+n1]); ll = edb[i0:i0+n1]
    m = vv > 0.02
    X += list(20*np.log10(vv[m]/st[2])); Y += list(ll[m] - np.median(edb[i0:i0+int(st[1]*fs)]))
X=np.array(X);Y=np.array(Y)
print("level (dB re stroke) vs speed (dB re plateau): slope %.2f" % np.polyfit(X, Y, 1)[0], "n", len(X))
