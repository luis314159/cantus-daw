"""The representation ladder (round 40, ladder): each *recorded* hit resynthesised through
richer and richer descriptions, to find the cheapest one listeners and measures cannot
tell from the recording (research-p3/drum-strategies.md, strategy A, §3.0).

    python -I ladder.py <out dir> [kit ...] [art ...]

Rungs, per hit (mono, the hits file's two channels averaged as every measure here does):

- R0  the recording;
- R1  model 5's description of that very hit: its fitted modes (drums with their onset
      phases and early parts), 38 quarter-octave noise bands at 22 breakpoints and the
      generic stick — the fits the engine's tables were built from (fit9 / fitv3, the ride
      refitted at 440 partials), rendered as synth.py renders them;
- R2  R1 with the stick replaced by a minimum-phase attack built from the hit's own first
      20 ms (its energy spectrum less the modes' and the late noise's), the bands faded in
      over 10-20 ms;
- R3  R2 with the noise as a fine table: 1/12-octave bands (~110, 35 Hz-21 kHz) x 40
      log-spaced times (0.5 ms-4 s) of the residual's power (the recording's STFT less the
      modes' and the attack's), random phase — what a run-time "model 6" table can hold;
- R3o the oracle of magnitude-only noise: the residual's own STFT magnitude per frame and
      bin (1024/256), random phase. Not a description a table can hold (it is a magnitude
      spectrogram of the hit): it is the ceiling of every phase-free noise layer;
- R4  R3 plus texture statistics: K latent modulators (the excess cross-band covariance of
      the residual's log-envelopes over R3's, as loadings, each driven by a seeded Gaussian
      process with its measured modulation spectrum), partials with their measured width
      (a slow seeded FM wobble, cymbals and hats) and, for wires and hat plates, a
      PhISEM-style burst stream; three scalars per hit chosen against the hit's own
      co-modulation, fluctuation and envelope kurtosis;
- R5  the anchor: R1 low-passed at 3.5 kHz.

Every random draw is seeded from the hit's name and the rung; every rung is written as a
24-bit WAV `<kit>-<art>-v<vel>-<rung>.wav`, plus `ladder.json` (what each hit's R4 chose).
Off-line only, never shipped; numpy, scipy, soundfile.
"""
import json
import os
import sys
import zlib
import numpy as np
import soundfile as sf
from scipy import signal

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from metrics import SR, load, hit  # noqa: E402
import synth  # noqa: E402
from early import onset1  # noqa: E402

TMP = "F:/projects/daw-r40/tmp/"
KITS = {
    "drs": dict(name="DRSKit", hits=TMP + "meas9/pack-drskit-tympanum-kit-hits.flac",
                half=TMP + "meas11/o50/pack-drskit-tympanum-kit-hits.wav",
                fit=TMP + "dg/out/fit9.json", ride=TMP + "dg/out/fit_ride.json"),
    "virt": dict(name="Virtuosity", hits=TMP + "meas9/pack-virtuosity-drums-kit-hits.flac",
                 half=TMP + "dg/vo50/pack-virtuosity-drums-kit-hits.wav",
                 fit=TMP + "dg/out/fitv3.json", ride=TMP + "dg/out/fitv_ride.json"),
}
# (articulation, the piece in the hits file, the fit's key, the hits file, clip length s)
ARTS = [
    ("kick", "kick", "kick", "hits", 1.5),
    ("snare", "snare", "snare", "hits", 1.5),
    ("sidestick", "sidestick", "sidestick", "hits", 1.0),
    ("rimshot", "rimshot", "rimshot", "hits", 1.5),
    ("tom_mid", "tom_mid", "tom_mid", "hits", 2.0),
    ("tom_floor", "tom_floor", "tom_floor", "hits", 2.5),
    ("hat_closed", "hat_closed", "hat_closed", "hits", 0.8),
    ("hat_half", "hat_open", "hat_o50", "half", 2.0),
    ("hat_open", "hat_open", "hat_open", "hits", 3.0),
    ("ride", "ride", "ride", "hits", 3.5),
    ("ride_bell", "ride_bell", "ride_bell", "hits", 3.5),
    ("crash", "crash", "crash", "hits", 3.8),
]
ART = {a[0]: a for a in ARTS}
DRUMS = {"kick", "snare", "sidestick", "rimshot", "tom_mid", "tom_floor"}
HATS = {"hat_closed", "hat_half", "hat_open"}
CYMS = HATS | {"ride", "ride_bell", "crash"}
BURSTS = {"snare", "rimshot"} | HATS          # wires and plates: event streams in R4
VELS = [80, 127]                              # the hits files hold 40/80/110/127
ALLV = [40, 80, 110, 127]
RUNGS = ["R0", "R1", "R2", "R3", "R3o", "R4", "R5"]
TA = 0.020                                    # the attack's window, s
NFFT, HOP = 1024, 256
PRE = 0.02                                    # the hit's pre-roll in `hit()`, s


def seed(*keys):
    return zlib.crc32("|".join(str(k) for k in keys).encode())


_X = {}
_FIT = {}


def recorded(kit, art, vel):
    """The recorded hit (mono), its onset (5 % detector, as the fits used) and length."""
    a = ART[art]
    path = KITS[kit][a[3]]
    if path not in _X:
        _X[path] = load(path)
    m, on = hit(_X[path], a[1], vel, length=a[4] + 0.05, pre=PRE)
    # (each hit starts its 4-s slot: 20 ms of silence before it, for the clip's pre-roll)
    pad = max(0, int(PRE * SR) - on)
    return np.concatenate([np.zeros(pad), m]), on + pad


def layer(kit, art, vel):
    """Model 5's fitted layer for this hit (the fits' level equals the hit's to 0.01 dB)."""
    a = ART[art]
    key = KITS[kit]["ride" if a[2] == "ride" else "fit"]
    if key not in _FIT:
        _FIT[key] = json.load(open(key))
    tab = _FIT[key]
    return tab, tab["pieces"][a[2]][ALLV.index(vel)]


# ---- R1: model 5's synthesis, by parts (synth.render's algorithm) -----------------------

def render_bands(tab, lay, n, rng):
    env = np.array(lay["env"], dtype=float)
    bp = np.array(tab["bp_ms"], dtype=float)
    noise = rng.uniform(-1, 1, n) * np.sqrt(3)
    t = np.arange(n) / SR * 1000
    out = np.zeros(n)
    for k, c in enumerate(tab["centres"]):
        if c * 2 ** (1 / 8) > 0.47 * SR or env[k].max() < -150:
            continue
        y = synth.band_filter(c, noise) * synth.band_norm(c)
        db = env[k] + rng.normal(0, 0.7)
        e = np.interp(t, bp, db)
        slope = (db[-1] - db[-2]) / (bp[-1] - bp[-2])
        e = np.where(t > bp[-1], db[-1] + slope * (t - bp[-1]), e)
        out += y * 10 ** (e / 20)
    return out


def jitter_noises(n, rng, k=4, hz=3.0):
    """K shared slow unit-variance noises (a partial's wobble mixes them)."""
    d = 480
    m = n // d + 8
    sos = signal.butter(2, hz, fs=SR / d, output="sos")
    z = signal.sosfiltfilt(sos, rng.normal(0, 1, (k, m)), axis=1)
    z /= z.std(1, keepdims=True) + 1e-12
    tt = np.arange(m) * d
    return np.array([np.interp(np.arange(n), tt, zz) for zz in z])


def render_modes(lay, n, rng, widths=None, jrng=None):
    """The modes (and a drum's early parts). `widths` (Hz, FWHM per mode): each partial's
    frequency wobbles slowly by a Gaussian of that width (a mix of K shared noises)."""
    modes = lay["modes"]
    t = np.arange(n) / SR
    gl_g, gl_tau = lay.get("glide", [0.0, 0.05])
    onset = lay.get("ets", 0.0) > 0 and len(lay.get("ph", [])) == len(modes)
    base = t + gl_g * gl_tau * (1 - np.exp(-t / gl_tau))
    if widths is not None:
        J = jitter_noises(n, jrng)
        Jint = np.cumsum(J, axis=1) / SR           # the integral of each shared noise
    out = np.zeros(n)
    for j, (hz, amp, t60, rise, direct) in enumerate(modes):
        ph = lay["ph"][j] if onset else rng.uniform(0, 2 * np.pi)
        g = amp * 10 ** (rng.normal(0, 1.5) / 20)
        e = np.exp(-6.9078 * t / t60)
        if rise > 0 and not onset:
            e = e - (1 - direct) * np.exp(-t * (6.9078 / t60 + 1 / rise))
        arg = 2 * np.pi * hz * base
        if widths is not None and widths[j] > 0:
            w = jrng.normal(0, 1, len(J))
            w /= np.linalg.norm(w)
            # a Gaussian wobble of deviation D has a -6 dB (power -3 dB) width ~2.355 D
            arg = arg + 2 * np.pi * (widths[j] / 2.355) * (w @ Jint)
        out += g * e * np.cos(arg + ph)
        if onset:
            out += lay["ea"][j] * np.exp(-t / lay["ets"]) * np.cos(arg + lay["ep"][j])
    return out


def render_stick(lay, vel, n, rng):
    st = lay.get("stick", [0.0, 0.0])
    return synth.stick(st, vel, n, rng) if max(st) > 0 else np.zeros(n)


def place(sig, n, at):
    """`sig` starting at sample `at` of an n-sample clip."""
    y = np.zeros(n)
    k = min(len(sig), n - at)
    y[at:at + k] = sig[:k]
    return y


def onset_gate(n, at):
    """0 before `at`, 1 ms up: a random-phase STFT layer's frames would ring before the hit."""
    return np.clip((np.arange(n) - at) / (0.001 * SR), 0, 1)


def late_gate(n, at):
    """0 before TA/2 after `at`, a raised cosine to 1 at TA: where the attack hands over."""
    t = (np.arange(n) - at) / SR
    g = np.clip((t - TA / 2) / (TA / 2), 0, 1)
    return 0.5 - 0.5 * np.cos(np.pi * g)


# ---- R2: the minimum-phase attack -----------------------------------------------------

def minphase(mag, nfft):
    """The minimum-phase signal of a magnitude (rfft bins of nfft): the folded cepstrum."""
    c = np.fft.irfft(np.log(np.maximum(mag, 1e-12)), nfft)
    f = np.zeros(nfft)
    f[0] = c[0]
    f[1:nfft // 2] = 2 * c[1:nfft // 2]
    f[nfft // 2] = c[nfft // 2]
    return np.fft.irfft(np.exp(np.fft.rfft(f)), nfft)


GRAINS_MS = [0.0, 1.5, 3.0, 6.0, 10.0, 20.0]


def attack(m, a0, others, coherent=None, nfft=4096):
    """The hit's first TA as minimum-phase grains: per window (0-1.5, 1.5-3, 3-6, 6-10,
    10-20 ms from a0) a pulse with the energy spectrum the recording has there beyond
    `others` (the late noise, random-phase modes: in power) and `coherent` (a drum's modes
    with their fitted onset phases: as a waveform) and the grains before it; each pulse twice its
    window long (at least 3 ms), its last quarter tapered. Magnitudes only: no phase of
    the recording is kept. Returns the attack placed in the clip."""
    n = len(m)
    out = np.zeros(n)
    # high-passed first: a short segment's edges leak a drum's strong lows into its quiet
    # highs, which the grains would then carry
    hp = signal.butter(4, 250, btype="high", fs=SR, output="sos")
    m = signal.sosfiltfilt(hp, m)
    others = signal.sosfiltfilt(hp, others)
    if coherent is not None:
        coherent = signal.sosfiltfilt(hp, coherent)
    for t0, t1 in zip(GRAINS_MS[:-1], GRAINS_MS[1:]):
        s0, s1 = a0 + int(t0 * SR / 1000), a0 + int(t1 * SR / 1000)
        seg = lambda y: np.fft.rfft(y[s0:s1], nfft)
        P0 = np.abs(seg(m)) ** 2
        if coherent is None:
            P = P0 - np.abs(seg(others + out)) ** 2
        else:
            # where the fitted onset phases hold (early.py: below ~1.5 kHz, the first few
            # ms) the modes come off as a waveform, elsewhere in power: per bin the less
            P = (np.minimum(np.abs(seg(m - coherent)) ** 2, P0 - np.abs(seg(coherent)) ** 2)
                 - np.abs(seg(others + out)) ** 2)
        Pa = np.maximum(P, 0.02 * P0)
        Pa = np.convolve(Pa, np.ones(3) / 3, mode="same")
        # above ~300 Hz only (the modes and their early parts carry the lows; a pulse a few
        # ms long cannot, and cut short it smears broadband), 80 dB of range
        f = np.fft.rfftfreq(nfft, 1 / SR)
        Pa = Pa * np.clip((f - 200) / 200, 0, 1) ** 2
        Pa = np.maximum(Pa, Pa.max() * 1e-8)
        h = minphase(np.sqrt(Pa), nfft)[:max(int(2 * (t1 - t0) * SR / 1000), int(0.003 * SR))]
        q = len(h) // 4
        h[-q:] *= 0.5 + 0.5 * np.cos(np.pi * np.arange(q) / q)
        out += place(h, n, s0)
    return out


# ---- STFT (Hann, 1024/256, frames centred on i*HOP) ----------------------------------

WIN = np.hanning(NFFT + 1)[:NFFT]
FREQ = np.fft.rfftfreq(NFFT, 1 / SR)


def stft(x):
    n = len(x)
    xp = np.pad(x, (NFFT // 2, NFFT))
    nf = n // HOP + 1
    idx = np.arange(NFFT)[None, :] + HOP * np.arange(nf)[:, None]
    return np.fft.rfft(xp[idx] * WIN, axis=1)


def istft(X, n):
    nf = X.shape[0]
    fr = np.fft.irfft(X, NFFT, axis=1) * WIN
    y = np.zeros(nf * HOP + NFFT)
    w2 = np.zeros_like(y)
    for i in range(nf):
        y[i * HOP:i * HOP + NFFT] += fr[i]
        w2[i * HOP:i * HOP + NFFT] += WIN ** 2
    y = y / np.maximum(w2, 1e-3)
    return y[NFFT // 2:NFFT // 2 + n]


def frame_times(nf, at):
    return (np.arange(nf) * HOP - at) / SR


THIRD = 1000 * 2 ** (np.arange(-10, 14) / 3)            # 100 Hz .. 20 kHz
THIRD = THIRD[THIRD < 0.45 * SR]


def bands_of(P, centres, width):
    """Mean power per band (bins within `width` octaves around each centre; the nearest
    bin when none) -> (frames x bands)."""
    out = np.empty((P.shape[0], len(centres)))
    for b, c in enumerate(centres):
        k = (FREQ >= c * 2 ** (-width / 2)) & (FREQ < c * 2 ** (width / 2))
        if not k.any():
            k = np.argmin(np.abs(FREQ - c)) == np.arange(len(FREQ))
        out[:, b] = P[:, k].mean(1)
    return out


def to_bins(B, centres):
    """Band values (dB, frames x bands) to every bin, interpolated on log frequency."""
    lf = np.log2(np.maximum(FREQ, 1.0))
    lc = np.log2(centres)
    return np.array([np.interp(lf, lc, row) for row in B])


def noise_resynth(Ptarget, n, rng, iters=4, parts=False):
    """Random-phase noise whose STFT power follows Ptarget (frames x bins): the phases are
    drawn once and kept; only a smooth gain (5 frames x 3 bins) is corrected after
    re-analysis — never the analysis's phases (that would be Griffin-Lim)."""
    ph = np.exp(2j * np.pi * rng.uniform(0, 1, Ptarget.shape))
    A = np.sqrt(Ptarget)
    ker = np.ones((5, 3)) / 15
    sm = lambda X: signal.fftconvolve(X, ker, mode="same")
    tb = np.maximum(sm(Ptarget), 1e-30)
    for _ in range(iters):
        Q = np.abs(stft(istft(A * ph, n))[:len(A)]) ** 2
        r = 10 * np.log10(tb / np.maximum(sm(Q), 1e-30))
        A = A * 10 ** (np.clip(r, -12, 12) / 20)
    # and a last broadband gain per frame (5-frame means): the bin-wise loop settles
    # ~0.8 dB high on a steep decay
    for _ in range(2):
        Q = np.abs(stft(istft(A * ph, n))[:len(A)]) ** 2
        k5 = np.ones(5) / 5
        g = np.convolve(Ptarget.sum(1), k5, "same") / np.maximum(np.convolve(Q.sum(1), k5, "same"), 1e-30)
        A = A * np.sqrt(np.clip(g, 0.25, 4.0))[:, None]
    y = istft(A * ph, n)
    return (y, A, ph) if parts else y


# ---- R3: the fine table ------------------------------------------------------------------

TWELFTH = 35.0 * 2 ** (np.arange(0, 200) / 12)
TWELFTH = TWELFTH[TWELFTH < 21000]                  # 110 bands
TT = np.concatenate([[0.0], np.geomspace(0.0005, 4.0, 39)])   # 40 times, s


def residual(m, parts, at):
    """The recording's STFT power less the given parts' (the modes', the attack's): the
    noise the table describes; never under 3 % of the recording's."""
    P = np.abs(stft(m)) ** 2
    Po = sum(np.abs(stft(p)) ** 2 for p in parts)
    return P, np.maximum(P - Po, 0.03 * P)


def table_of(Pres, at):
    """(110 bands x 40 times) dB: the residual averaged over each 1/12 octave and, around
    each time, over max(one hop, 0.3 t) — fit.py's breakpoint rule."""
    B = bands_of(Pres, TWELFTH, 1 / 12)
    ft = frame_times(len(B), at)
    T = np.empty((len(TT), len(TWELFTH)))
    for j, t in enumerate(TT):
        w = max(HOP / SR, 0.3 * t)
        k = np.abs(ft - t) <= w / 2
        if not k.any():
            k = np.argmin(np.abs(ft - t)) == np.arange(len(ft))
        T[j] = B[k].mean(0)
    return 10 * np.log10(T + 1e-30)


def table_power(T, nf, at):
    """The table read back at every frame (dB interpolated on log time) and bin."""
    ft = np.clip(frame_times(nf, at), 0, None)
    lt = np.log(TT + 0.0005)
    B = np.array([np.interp(np.log(ft + 0.0005), lt, T[:, b]) for b in range(T.shape[1])]).T
    # past the last time the last slope continues
    late = ft > TT[-1]
    if late.any():
        s = (T[-1] - T[-2]) / (TT[-1] - TT[-2])
        B[late] = T[-1] + (ft[late, None] - TT[-1]) * s
    B[frame_times(nf, at) < -0.005] = -300
    return 10 ** (to_bins(B, TWELFTH) / 10)


# ---- R4: texture statistics ------------------------------------------------------------

LAT = THIRD[THIRD >= 500]                       # the latent loadings' bands
K_LAT = 6


def log_dev(P, at, art):
    """Per 1/3 octave from 500 Hz, the log power (dB) per frame less its 165-ms running
    mean, inside the texture window: what the smooth table does not hold."""
    from ladder_measure import TEX
    from scipy.ndimage import uniform_filter1d
    L = 10 * np.log10(bands_of(P, LAT, 1 / 3) + 1e-30)
    D = L - uniform_filter1d(L, 31, axis=0, mode="nearest")
    ft = frame_times(len(P), at)
    a, b = TEX[art]
    return D[(ft >= max(a, TA)) & (ft <= b)]


def latents(Dr, D3, n_frames, rng):
    """Loadings from the recording's excess covariance over R3's (independent noise under
    the same envelopes), and seeded Gaussian drivers with each component's modulation
    spectrum -> per-frame band gains (dB, frames x LAT bands), mean power preserved."""
    C = np.cov(Dr.T) - np.cov(D3.T)
    lam, V = np.linalg.eigh(C)
    o = np.argsort(lam)[::-1][:K_LAT]
    lam, V = lam[o], V[:, o]
    keep = lam > 0
    lam, V = lam[keep], V[:, keep]
    G = np.zeros((n_frames, len(LAT)))
    info = dict(eig=[round(float(x), 3) for x in lam],
                share=float(lam.sum() / max(np.trace(np.cov(Dr.T)), 1e-9)))
    for k in range(len(lam)):
        p = Dr @ V[:, k]
        p = (p - p.mean()) * np.hanning(len(p))
        S = np.abs(np.fft.rfft(p, 2 * n_frames)) ** 2
        S = np.convolve(S, np.ones(3) / 3, mode="same")
        z = np.fft.irfft(np.sqrt(S) * np.exp(2j * np.pi * rng.uniform(0, 1, len(S))))[:n_frames]
        z /= z.std() + 1e-12
        G += np.outer(z, V[:, k] * np.sqrt(lam[k]))
    var = (V ** 2 * lam).sum(1)
    G -= np.log(10) / 10 * var / 2                  # E[10^(X/10)] = 1
    return G, info


def lat_gain_bins(G, alpha, n_frames, at):
    """Band gains to bins (dB): none below 400 Hz, none before the attack hands over."""
    g = to_bins(np.clip(alpha * G, -12, 12), LAT)
    g[:, FREQ < 400] *= 0
    g[frame_times(n_frames, at) < TA] = 0
    return g


def burst_stream(n, at, rng, rate, Phf):
    """PhISEM-style events: a Poisson stream (rate per second at the noise's peak, following
    its envelope), each event an exponentially distributed energy into a 0.7-ms decay, the
    sum times white noise; shaped in the STFT to `Phf` (frames x bins) over 48 ms, so the
    stream carries that power on average and its bursts on top."""
    env = np.sqrt(Phf.sum(1))
    env = env / (env.max() + 1e-30)
    lam = np.interp(np.arange(n), np.arange(len(env)) * HOP - NFFT // 2 + HOP, env) * rate / SR
    ev = (rng.uniform(0, 1, n) < lam) * rng.exponential(1.0, n)
    k = np.exp(-np.arange(int(0.005 * SR)) / (0.0007 * SR))
    e = signal.fftconvolve(ev, k)[:n]
    b = rng.normal(0, 1, n) * np.sqrt(np.maximum(e, 0))
    B = stft(b)[:len(Phf)]
    Q = np.abs(B) ** 2
    Qs = signal.convolve2d(bands_of(Q, THIRD, 1 / 3), np.ones((9, 1)) / 9, mode="same", boundary="symm")
    Ts = bands_of(Phf, THIRD, 1 / 3)
    g = to_bins(10 * np.log10((Ts + 1e-30) / (Qs + 1e-30)), THIRD)
    return istft(B * 10 ** (np.clip(g, -120, 80) / 20), n)


def mode_widths(m, on, lay, art):
    """Each partial's measured width beyond what its window and decay give a pure decaying
    sinusoid (-3 dB power widths in quadrature), Hz, 0..12; 0 where neighbours crowd."""
    import fit
    spec = fit.SPEC["hat_open" if art == "hat_half" else ART[art][2]]
    a, b = spec["win"]
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w)
    z = 8
    P = np.abs(np.fft.rfft(w * np.hanning(n), z * n)) ** 2
    df = SR / (z * n)
    cache = {}

    def fwhm(Q, i):
        h = Q[i] / 2
        lo = i
        while lo > 0 and Q[lo] > h:
            lo -= 1
        hi = i
        while hi < len(Q) - 1 and Q[hi] > h:
            hi += 1
        return (hi - lo) * df

    out = []
    tt = np.arange(n) / SR + a
    for hz, amp, t60, rise, direct in lay["modes"]:
        key = round(np.log(t60) * 20)
        if key not in cache:
            s = np.exp(-6.9078 * tt / t60) * np.cos(2 * np.pi * 1000.3 * tt)
            Q = np.abs(np.fft.rfft(s * np.hanning(n), z * n)) ** 2
            cache[key] = fwhm(Q, int(np.argmax(Q)))
        i0 = int(round(hz / df))
        r = max(1, int(2.0 / df))
        i = i0 - r + int(np.argmax(P[i0 - r:i0 + r + 1]))
        W = fwhm(P, i)
        W0 = cache[key]
        out.append(0.0 if W > 20 else float(np.clip(np.sqrt(max(W * W - W0 * W0, 0.0)), 0, 12)))
    return out


# ---- the clip ----------------------------------------------------------------------------

def clip_window(n, a0):
    """Every rung's clip: from 3 ms before the first sound (1-ms fade in) to the end
    (50-ms fade out)."""
    g = np.zeros(n)
    s = max(0, a0 - int(0.003 * SR))
    g[s:] = 1
    k = int(0.001 * SR)
    g[s:s + k] = np.linspace(0, 1, k)
    q = int(0.05 * SR)
    g[-q:] *= np.linspace(1, 0, q)
    return g, s


def build(kit, art, vel):
    """Every rung of one recorded hit: {rung: signal}, the clip's onset, and what R4 chose."""
    from ladder_measure import xcorr, fluct, kurt
    m, on = recorded(kit, art, vel)
    n = len(m)
    tab, lay = layer(kit, art, vel)
    a0 = onset1(m, on) - int(0.0005 * SR)
    s0 = seed(kit, art, vel)
    rng = lambda k: np.random.default_rng(s0 + k)
    # R1: model 5's parts, each with its own seed (every later rung reuses them)
    bands = place(render_bands(tab, lay, n - on, rng(1)), n, on)
    modes = place(render_modes(lay, n - on, rng(2)), n, on)
    stick = place(render_stick(lay, vel, n - on, rng(3)), n, on)
    R = {"R0": m.copy(), "R1": bands + modes + stick}
    # R2: the stick gives way to the hit's own minimum-phase attack
    gate = late_gate(n, a0)
    phased = lay.get("ets", 0.0) > 0 and len(lay.get("ph", [])) == len(lay["modes"])
    att = attack(m, a0, bands * gate, modes) if phased else attack(m, a0, modes + bands * gate)
    R["R2"] = modes + bands * gate + att
    # R3 / R3o: the residual's fine table / its own frames, random phase
    P, Pres = residual(m, [modes, att], a0)
    nf = len(P)
    T = table_of(Pres, a0)
    P3 = table_power(T, nf, a0)
    # (the table's residual has the attack's power out already: only the onset is gated)
    gate = onset_gate(n, a0)
    noise3, A3, ph3 = noise_resynth(P3, n, rng(4), parts=True)
    R["R3"] = modes + att + noise3 * gate
    R["R3o"] = modes + att + noise_resynth(Pres, n, rng(5)) * gate
    # R4: latent co-modulation, partial widths, bursts
    Dr = log_dev(Pres, a0, art)
    D3 = log_dev(np.abs(stft(noise3 * gate)) ** 2, a0, art)
    G, lat = latents(Dr, D3, nf, rng(6))
    widths = mode_widths(m, on, lay, art) if art in CYMS else None
    modes4 = place(render_modes(lay, n - on, rng(2), widths, rng(7)), n, on) if widths else modes
    mask = np.clip((FREQ - 1000) / 1000, 0, 1)
    g, s = clip_window(n, a0)
    # (measured on the clip as written, as ladder_measure.py reads it)
    clip = lambda y: np.pad((y * g)[s:], (0, SR))
    stats = lambda y: (xcorr(clip(y), on - s, art), xcorr(clip(y), on - s, art, 40, 150),
                       fluct(clip(y), on - s, art, 8000, 16000), kurt(clip(y), on - s, art))
    want = stats(m)
    streams = {}
    best = None
    opts = [(0.0, 0)] + ([(0.3, 400), (0.3, 3000), (0.6, 400), (0.6, 3000)] if art in BURSTS else [])
    textured = art in BURSTS or art in CYMS
    for alpha in ((0.0, 0.5, 1.0, 1.5, 2.0, 3.0) if len(lat["eig"]) and textured else (0.0,)):
        gl = 10 ** (lat_gain_bins(G, alpha, nf, a0) / 20)
        for beta, rate in opts:
            Y = A3 * ph3 * gl * np.sqrt(1 - beta * mask)
            y = istft(Y, n)
            if beta:
                if rate not in streams:
                    streams[rate] = burst_stream(n, a0, rng(8 + rate), rate, P3 * mask)
                y = y + np.sqrt(beta) * streams[rate]
            sig = modes4 + att + y * gate
            got = stats(sig)
            score = (abs(got[0] - want[0]) / 0.05 + abs(got[1] - want[1]) / 0.05
                     + abs(got[2] - want[2]) / 0.3 + abs(got[3] - want[3]) / 0.5)
            if best is None or score < best[0]:
                best = (score, alpha, beta, rate, sig, got)
    R["R4"] = best[4]
    # R5: the anchor
    R["R5"] = signal.sosfiltfilt(signal.butter(8, 3500, fs=SR, output="sos"), R["R1"])
    R = {k: (v * g)[s:] for k, v in R.items()}
    info = dict(onset=on - s, first=a0 - s, alpha=best[1], beta=best[2], rate=best[3],
                r4_score=round(best[0], 2), want=[round(x, 3) for x in want],
                got=[round(x, 3) for x in best[5]], latent=lat,
                widths=None if widths is None else dict(
                    median=round(float(np.median(widths)), 2),
                    share_wide=round(float(np.mean(np.array(widths) > 1.0)), 2), n=len(widths)),
                attack_db=round(10 * np.log10(np.sum(att ** 2) / (np.sum(m[a0:a0 + int(TA * SR)] ** 2) + 1e-30)), 2))
    return R, info


def job(args):
    out, kit, art, vel = args
    R, info = build(kit, art, vel)
    peak = max(np.abs(v).max() for v in R.values())
    g = min(1.0, 0.95 / peak)
    for k, v in R.items():
        sf.write(os.path.join(out, f"{kit}-{art}-v{vel}-{k}.wav"), v * g, SR, subtype="PCM_24")
    info["gain"] = g
    print(f"{kit:4s} {art:10s} v{vel:3d}  R4 alpha {info['alpha']} beta {info['beta']} rate {info['rate']} "
          f"want {info['want']} got {info['got']}", flush=True)
    return f"{kit}-{art}-v{vel}", info


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    kits = [a for a in sys.argv[2:] if a in KITS] or list(KITS)
    arts = [a for a in sys.argv[2:] if a in ART] or [a[0] for a in ARTS]
    jobs = [(out, k, a, v) for k in kits for a in arts for v in VELS]
    from multiprocessing import Pool
    with Pool(int(os.environ.get("LADDER_JOBS", "6"))) as pool:
        res = dict(pool.map(job, jobs, chunksize=1))
    path = os.path.join(out, "ladder.json")
    old = json.load(open(path)) if os.path.exists(path) else {}
    old.update(res)
    json.dump(old, open(path, "w"), indent=1)


if __name__ == "__main__":
    main()
