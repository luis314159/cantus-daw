"""paganini_low.py (r42 arcus-classical pass 2): <W> [names...]: per note (aligned take, p_on..p_off), the 150-400 Hz band (minus
the note's own partials) re the note's partial sum, for high notes (key>=81) and the fundamental
re the 2nd-4th partials for low notes (key<=69); original vs renders in <W>. Also a third-octave
LTAS difference 1-16 kHz."""
import sys, json, numpy as np, soundfile as sf
from scipy import signal
W = sys.argv[1]
names = sys.argv[2:] or ["arcus-mejora-alineado-1cuerda-sala"]
notes = json.load(open(f"{W}/work/notes.json"))["notes"]
def load(p):
    x, sr = sf.read(p); x = x.mean(axis=1) if x.ndim > 1 else x
    return x, sr
def spec(x, sr, a, b):
    s = x[int(a*sr):int(b*sr)]
    if len(s) < 2048: return None, None
    f, p = signal.welch(s, sr, nperseg=4096 if len(s) >= 4096 else 2048)
    return f, p
def note_stats(x, sr):
    hi, lo = [], []
    for n in notes:
        a, b = n["p_on"] + 0.03, n["p_off"]
        if b - a < 0.08: continue
        f, p = spec(x, sr, a, b)
        if f is None: continue
        f0 = 440 * 2 ** ((n["key"] - 69) / 12)
        harm = np.zeros_like(f, bool)
        for k in range(1, 40):
            harm |= np.abs(f - k * f0) < 0.06 * f0
        tot = p[harm].sum()
        if n["key"] >= 81:
            band = (f >= 150) & (f <= 400) & ~harm
            hi.append(10 * np.log10(p[band].sum() / tot + 1e-20))
        if n["key"] <= 69:
            def pk(k): return p[np.abs(f - k * f0) < 0.06 * f0].sum()
            lo.append(10 * np.log10(pk(1) / (pk(2) + pk(3) + pk(4)) + 1e-20))
    return hi, lo
def ltas(x, sr):
    f, p = signal.welch(x, sr, nperseg=8192)
    out = []
    for fc in [1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000]:
        m = (f >= fc / 2 ** (1/6)) & (f < fc * 2 ** (1/6))
        out.append(10 * np.log10(p[m].mean() + 1e-20))
    return np.array(out)
xo, sr = load(f"{W}/original.wav")
ho, lo_ = note_stats(xo, sr); Lo = ltas(xo, sr)
print(f"original: hi-notes 150-400 re partials median {np.median(ho):.1f} dB (n {len(ho)}); low notes H1 re H2-4 {np.median(lo_):.1f} (n {len(lo_)})")
for nm in names:
    x, sr2 = load(f"{W}/{nm}.wav")
    h, l = note_stats(x, sr2); L = ltas(x, sr2)
    # level-match ltas at 1 kHz-4 kHz mean
    d = (L - Lo); d -= np.mean(d[:7])
    print(f"{nm}: hi {np.median(h):.1f} (re orig {np.median(h)-np.median(ho):+.1f}); low H1 {np.median(l):.1f} ({np.median(l)-np.median(lo_):+.1f}); ltas 1k..16k re orig (1-4k matched): " + " ".join(f"{v:+.1f}" for v in d))
