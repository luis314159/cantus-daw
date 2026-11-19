"""Comb/early-reflection signature (cepstrum, fine spectral ripple), tom sympathetic buzz,
snare wire spectrum and grain, drum upper-mode ring at 500 ms."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
from scipy import ndimage


def ripple(m, on, a=0, b=60, lo=300, hi=4000):
    y = np.pad(m, (0, SR))[on + int(a * SR / 1000):on + int(b * SR / 1000)]
    n = 1 << 15
    X = 10 * np.log10(np.abs(np.fft.rfft(y * np.hanning(len(y)), n)) ** 2 + 1e-20)
    f = np.fft.rfftfreq(n, 1 / SR)
    k = (f >= lo) & (f < hi)
    # residual about a 1/3-octave smoothing (log-frequency resample)
    lf = np.geomspace(lo, hi, 600)
    Xi = np.interp(lf, f[k], X[k])
    sm = ndimage.uniform_filter1d(Xi, 600 // int(np.log2(hi / lo) * 3))
    return float(np.std(Xi - sm))


def cep_peak(m, on, a=0, b=80):
    """Largest real-cepstrum peak at 0.7-15 ms quefrency (an echo/comb), in its own units."""
    y = np.pad(m, (0, SR))[on + int(a * SR / 1000):on + int(b * SR / 1000)]
    n = 1 << 14
    L = np.log(np.abs(np.fft.rfft(y * np.hanning(len(y)), n)) + 1e-9)
    c = np.fft.irfft(L - L.mean())
    q = np.arange(len(c)) / SR * 1000
    k = (q > 0.7) & (q < 15)
    i = np.argmax(np.abs(c[k]))
    return float(np.abs(c[k][i]) / np.std(c[k])), float(q[k][i])


if __name__ == "__main__":
    print("== fine spectral ripple 300-4000 Hz, 0-60 ms (dB sd about 1/3-oct smooth) and cepstral echo peak (z, ms); v80 v127")
    for el in ("bombo", "caja", "tom-medio", "tom-de-piso", "crash"):
        for kit in KITS:
            s = []
            for v in ("gen", "rec"):
                for vi in (1, 3):
                    m, on = element_hits(kit, el, v)[vi]
                    z, q = cep_peak(m, on)
                    s.append(f"{v} v{VELS[vi]} rip {ripple(m, on):4.1f} cep {z:4.1f}@{q:4.1f}ms")
            print(f"  {kit:6s} {el:12s} " + " | ".join(s))
        print("     kits v110: " + " ".join(f"{pk} rip {ripple(*pack_hit(pk, EL2P[el], 110)):4.1f} cep {cep_peak(*pack_hit(pk, EL2P[el], 110))[0]:4.1f}" for pk in PACKS))
    print("\n== toms: sympathetic buzz = 3-8 kHz in 100-400 ms re body 60-300 Hz (dB), flatness 3-8k; 250-1k and 1-4k at 500 ms re own peak")
    def tomx(m, on):
        m = np.pad(m, (0, SR))
        e = lambda lo, hi, a, b: 10 * np.log10(np.mean(bandpass(m, lo, hi)[on + int(a * SR / 1000):on + int(b * SR / 1000)] ** 2) + 1e-30)
        buzz = e(3000, 8000, 100, 400) - e(60, 300, 100, 400)
        fl = flat_band(m, on, 100, 400, 3000, 8000)
        r = []
        for lo, hi in ((250, 1000), (1000, 4000)):
            env = band_env(m, lo, hi, 0.02)
            r.append(20 * np.log10(env[on + int(0.5 * SR)] / env[on:on + int(0.1 * SR)].max()))
        return buzz, fl, r[0], r[1]
    for el in ("tom-medio", "tom-de-piso", "bombo"):
        for kit in KITS:
            g = [tomx(*element_hits(kit, el, "gen")[i]) for i in range(4)]
            r = [tomx(*element_hits(kit, el, "rec")[i]) for i in range(4)]
            for j, nm in enumerate(("buzz", "flat", "mid@500", "hi@500")):
                print(f"  {kit:6s} {el:12s} {nm:8s} gen " + " ".join(f"{x[j]:6.2f}" for x in g) + " | rec " + " ".join(f"{x[j]:6.2f}" for x in r))
        for pk in PACKS:
            x = tomx(*pack_hit(pk, EL2P[el], 110))
            print(f"     {pk:10s} v110 buzz {x[0]:6.1f} flat {x[1]:.2f} mid@500 {x[2]:6.1f} hi@500 {x[3]:6.1f}")
    print("\n== snare wires: 1/3-oct shape 1-16 kHz in 50-250 ms re its max (gen-rec), wire grain (4-10k envelope residual sd dB 20-250 ms)")
    def wires(m, on):
        m = np.pad(m, (0, SR))
        c, L = thirdoct_spec(m[on + int(0.05 * SR):on + int(0.25 * SR)])
        k = c >= 1000
        e = band_env(m, 4000, 10000, 0.001)
        seg = 20 * np.log10(e[on + int(0.02 * SR):on + int(0.25 * SR)])[::24]
        t = np.arange(len(seg)); p = np.polyfit(t, seg, 3)
        return c[k], L[k] - L[k].max(), float(np.std(seg - np.polyval(p, t)))
    for kit in KITS:
        for vi in range(4):
            cg, Lg, gg = wires(*element_hits(kit, "caja", "gen")[vi]); _, Lr, gr = wires(*element_hits(kit, "caja", "rec")[vi])
            print(f"  {kit:6s} v{VELS[vi]:3d} shape gen-rec " + " ".join(f"{x:4.0f}" for x in Lg - Lr) + f"  | grain gen {gg:4.2f} rec {gr:4.2f}")
    print("   bands: " + " ".join(f"{int(x)}" for x in cg))
    for pk in PACKS:
        _, L, g = wires(*pack_hit(pk, "snare", 110))
        print(f"     {pk:10s} v110 shape re max " + " ".join(f"{x:4.0f}" for x in L) + f" grain {g:4.2f}")
