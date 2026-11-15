"""Measure a recorded cymbal or hat into the "measured" voice's tables (round 40, tymp-engine).

    python -I fit.py <reference hits.flac> <out.json> [piece ...]

Per piece and velocity layer (the hits file's 40/80/110/127):

- **modes**: the partials that stand above the spectrum's local median in a long window
  (the tonal layer), each with its frequency, its amplitude at the strike and its T60,
  tracked through a long STFT; capped so no partial carries more than CAP_ONE of the
  energy above 1 kHz;
- **bands**: the rest as noise in quarter-octave bands, each band's envelope (dB re full
  scale RMS) at fixed breakpoints after the onset, with the modes' own power taken out;
- the energy below 450 Hz is held at the median recorded kit's (the reference's own low
  end is mostly the other drums' bleed and the room, analysis-drums/report.md item 7).

Nothing here is shipped; the JSON becomes a Rust table (tables.py).
"""
import json
import sys
import numpy as np
from scipy import signal, ndimage
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import SR, PIECES, VELS, load, hit

# Quarter-octave bands from 100 Hz: centres and edges.
NB = 38
# (down to 35 Hz: a kick's and a floor tom's low end is theirs)
CENTRES = [100.0 * 2 ** ((k - 6) / 4) for k in range(NB)]
# Breakpoints, ms after the onset.
BP_MS = [0, 0.5, 1, 2, 3, 5, 8, 12, 18, 27, 40, 60, 90, 135, 200, 300, 450, 700,
         1000, 1500, 2200, 3200]
CAP_ONE = 0.04
DEBUG = False
# A cymbal's noise capped by the recording's residual (drums-gen 2; `TYMP_FIT_RESID=0` off).
RESID = __import__("os").environ.get("TYMP_FIT_RESID", "1") != "0"
# A partial's T60 read only above the floor (drums-gen 2; `TYMP_FIT_FLOOR=0` off).
FLOOR_T60 = __import__("os").environ.get("TYMP_FIT_FLOOR", "1") != "0"
# What each piece keeps: the tonal layer's range and size, the window the modes are
# found in, s after the onset, and the energy below 450 Hz re the whole (dB).
# A drum's spec: its partials are its body (`ref` 30 Hz: no share is reckoned above 1 kHz
# only), its low end is its own (no `low_db`), its level the whole band's (`level_hp` 0),
# its pitch glides (`glide`).
DRUM = dict(win=(0.05, 1.05), ref=30, level_hp=0, glide=0.15, cap=0.6, med_hz=60)
SPEC = {
    # (the most a glide may be: a felt kick's little, a snare's less — the early window's
    # strongest partial is not always the late one's)
    # (drums-gen 2, cue 13: DRSKit's kick falls from ~110 to 76 Hz at v127 — its partials read
    # after the fall, its glide up to 45 %; read across the fall, a smeared 61 Hz led)
    "kick": dict(DRUM, fmin=30, fmax=2500, nmax=48, glide=0.45, win=(0.25, 1.25)),
    "snare": dict(DRUM, fmin=80, fmax=4000, nmax=48, glide=0.04, subcut=0.8),
    "rimshot": dict(DRUM, fmin=80, fmax=6000, nmax=48, glide=0.04, subcut=0.8),
    "sidestick": dict(DRUM, fmin=80, fmax=6000, nmax=40, glide=0.0),
    "tom_hi": dict(DRUM, fmin=40, fmax=3000, nmax=48, subcut=1.3),
    "tom_mid": dict(DRUM, fmin=40, fmax=3000, nmax=48, subcut=1.3),
    "tom_floor": dict(DRUM, fmin=30, fmax=3000, nmax=48, subcut=1.3),
    "crash": dict(fmin=120, fmax=16000, nmax=220, win=(0.1, 1.1), low_db=-18.0, cap=0.03),
    "ride": dict(fmin=120, fmax=16000, nmax=440, win=(0.1, 1.1), low_db=-9.5, cap=0.08),
    "ride_bell": dict(fmin=120, fmax=16000, nmax=200, win=(0.1, 1.1), low_db=-12.0, cap=0.22),
    "hat_open": dict(fmin=250, fmax=15000, nmax=140, win=(0.1, 0.6), low_db=-14.0, cap=0.025),
    "hat_closed": dict(fmin=300, fmax=15000, nmax=40, win=(0.01, 0.12), low_db=-14.0, cap=0.025),
    "hat_pedal": dict(fmin=300, fmax=15000, nmax=24, win=(0.01, 0.10), low_db=-8.0),
}


def edges(c):
    return c * 2 ** (-1 / 8), c * 2 ** (1 / 8)


def band_power(m, lo, hi):
    sos = signal.butter(3, [lo, min(hi, 23000)], btype="band", fs=SR, output="sos")
    y = signal.sosfiltfilt(sos, m)
    return y * y


def at_breakpoints(p, on):
    out = []
    for t in BP_MS:
        w = max(0.5, 0.3 * t) / 1000
        a = on + int((t / 1000 - w / 2) * SR) if t > 0 else on
        b = a + max(1, int(w * SR))
        out.append(float(np.mean(p[max(a, 0):b])))
    return np.array(out)


def find_modes(m, on, spec):
    a, b = spec["win"]
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w)
    P = np.abs(np.fft.rfft(w * np.hanning(n), 4 * n)) ** 2
    f = np.fft.rfftfreq(4 * n, 1 / SR)
    d = 10 * np.log10(P + 1e-30)
    med = ndimage.median_filter(d, size=int(spec.get("med_hz", 300) / f[1]) | 1)
    exc = d - med
    sel = (f >= spec["fmin"]) & (f <= spec["fmax"])
    pk, _ = signal.find_peaks(np.where(sel, exc, 0), height=10, distance=max(1, int(12 / f[1])))
    pk = pk[np.argsort(exc[pk])[::-1]][:spec["nmax"]]
    short = b < 0.3
    nper = 2048 if short else 8192
    hop = 256 if short else 1024
    ff, tt, S = signal.stft(m[on:on + int((0.4 if short else 3.0) * SR)], SR, nperseg=nper,
                            noverlap=nper - hop, boundary=None)
    tt = tt - nper / 2 / SR
    # A unit sinusoid's lobe in this window: the mean power a peak's lobe stands for.
    unit = np.abs(np.fft.rfft(np.cos(2 * np.pi * 1000.5 * np.arange(n) / SR) * np.hanning(n), 4 * n)) ** 2
    u = int(np.argmax(unit))
    unit_lobe = unit[u - 16:u + 17].sum() * 2
    tc = 0.5 * (a + b)
    modes = []
    for i in pk:
        j = int(np.argmin(np.abs(ff - f[i])))
        A = 2 * np.abs(S[max(0, j - 1):j + 2]).max(0)
        Adb = 20 * np.log10(A + 1e-12)
        k = (tt >= a) & (tt <= (b if short else 2.8))
        if k.sum() < 3:
            continue
        if FLOOR_T60:
            # Only while the partial stands 10 dB over the spectrum's floor around it (the
            # recording's hiss, the other mics): a slope read into the floor is a T60 far
            # too long (drums-gen 2: a jazz crash's highs rang 13 dB over the recording at
            # 2-3 s, the toms' upper modes 1-9 s against 0.4).
            w = max(2, int(200 / (ff[1] - ff[0])))
            # (the floor: the neighbourhood's median at its quietest — the hiss under it all)
            fl = 20 * np.log10(2 * np.median(np.abs(S[max(0, j - w):j + w + 1]), axis=0).min() + 1e-12)
            above = Adb > fl + 10
            idx = np.nonzero(k)[0]
            run = idx[:int(np.argmin(above[idx])) if not above[idx].all() else len(idx)]
            if len(run) >= 3:
                k = np.zeros_like(k)
                k[run] = True
            else:
                k = np.zeros_like(k)
                k[idx[:3]] = True
        sl, ic = np.polyfit(tt[k], Adb[k], 1)
        t60 = float(np.clip(-60 / sl if sl < 0 else 30.0, 0.05, 30.0))
        # The amplitude from the long window's lobe above the floor, at its centre,
        # carried back to the strike by the mode's own decay.
        lobe = max(P[i - 16:i + 17].sum() - 33 * 10 ** (med[i] / 10), 0.0)
        amp = np.sqrt(lobe / unit_lobe) * 10 ** (60 / 20 * tc / t60)
        modes.append([float(f[i]), float(amp), t60])
    return modes


def mode_env(md, t):
    """A mode's amplitude: its decay less the part that grows in (the bloom)."""
    hz, amp, t60, rise, direct = md
    e = np.exp(-6.9078 * t / t60)
    if rise > 0:
        e = e - (1 - direct) * np.exp(-t * (6.9078 / t60 + 1 / rise))
    return amp * e


def modal_power(modes, lo, hi, t):
    p = np.zeros_like(t)
    for md in modes:
        if lo <= md[0] < hi:
            p += 0.5 * mode_env(md, t) ** 2
    return p


def nearest_band(hz):
    return int(np.argmin([abs(np.log2(hz / c)) for c in CENTRES]))


def assign_rise(modes, totals):
    """Each mode grows in as its band's power does: a band that peaks late (the wash
    blooming) lends its modes a rise; `direct` is what the strike sets ringing at once."""
    t = np.array(BP_MS) / 1000
    out = []
    for hz, amp, t60 in modes:
        p = totals[nearest_band(hz)]
        if p is None:
            out.append([hz, amp, t60, 0.0, 1.0])
            continue
        # the band's peak after the stick's instant (the stick is its own part)
        i = int(np.argmax(np.where((t >= 0.003) & (t <= 1.0), p, 0)))
        tp = t[i]
        if tp < 0.012:
            out.append([hz, amp, t60, 0.0, 1.0])
            continue
        if tp > 0.8 * t60 / 6.9078:
            # it peaks after it would have decayed: a growing sizzle no decaying mode
            # can be; the band's noise carries it
            continue
        # what rings at once: the band's least between 2 and 18 ms (not the stick's
        # own instant) against its peak
        direct = float(np.clip(np.sqrt(np.min(p[3:9]) / p[i]), 0.0, 1.0))
        td = t60 / 6.9078
        best, rise = 1e9, tp
        for r in np.geomspace(0.002, 1.0, 200):
            tf = 1 / (1 / td + 1 / r)
            tt = np.log(td / tf) * td * tf / (td - tf)
            if abs(tt - tp) < best:
                best, rise = abs(tt - tp), r
        out.append([hz, amp, t60, float(rise), direct])
    return out


def clamp_t60(modes, totals, after=0.12, resid=None, drum=False):
    """A mode never outlasts its band: no more than 1.5 times the band's own decay, and
    its power under 80 % of the band's at every breakpoint after the window — or, where the
    recording's residual says the band is mostly partials (`resid`), under what the noise
    leaves (up to 97 %)."""
    t = np.array(BP_MS) / 1000
    out = []
    for md in modes:
        hz, amp, t60, rise, direct = md
        p = totals[nearest_band(hz)]
        if p is not None:
            # never much longer than its band's own decay after its peak
            d = 10 * np.log10(p + 1e-30)
            i = int(np.argmax(d))
            late = (t >= max(t[i], 0.3))
            if late.sum() >= 3:
                # its late slope: where a long tone, if it is there, has the band
                sl = np.polyfit(t[late], d[late], 1)[0]
                if sl < 0 and (drum or not (FLOOR_T60 and resid is not None)):
                    # (a T60 read above the floor stands: the longest partials are the tail)
                    t60 = min(t60, 1.5 * -60.0 / sl)
            lim = 0.8 if resid is None else np.clip(1.0 - resid[nearest_band(hz)], 0.8, 0.97)
            for _ in range(30):
                pw = 0.5 * mode_env([hz, amp, t60, rise, direct], t) ** 2
                late = t > after
                if np.all((pw <= lim * p + 1e-24)[late]):
                    break
                t60 *= 0.85
        out.append([hz, amp, float(max(t60, 0.02)), rise, direct])
    return out


def cap_modes(modes, m, on, spec):
    """No partial above CAP_ONE of the energy above 1 kHz in the window (its mean power)."""
    a, b = spec["win"]
    w = m[on + int(a * SR):on + int(b * SR)]
    sp = np.abs(np.fft.rfft(w)) ** 2
    f = np.fft.rfftfreq(len(w), 1 / SR)
    ref = spec.get("ref", 1000)
    hf = sp[f > ref].sum() / len(w) ** 2 * 2  # mean power above 1 kHz (a drum's: all)
    out = []
    tt = np.linspace(a, b, 64)
    for md in modes:
        hz, amp = md[0], md[1]
        mean = float(np.mean(0.5 * mode_env(md, tt) ** 2))
        one = spec.get("cap", CAP_ONE)
        lim = one * hf if hz > ref else 4 * one * hf
        if mean > lim:
            amp *= np.sqrt(lim / mean)
        out.append([hz, float(amp)] + list(md[2:]))
    # Nor do a band's modes carry more than 92 % of its power in the window.
    t = np.array(BP_MS) / 1000
    totals = band_totals(m, on)
    for k, c in enumerate(CENTRES):
        lo, hi = edges(c)
        p = totals[k]
        if p is None:
            continue
        mp = modal_power(out, lo, hi, t)
        inw = (t >= a) & (t <= b)
        r = float(np.sum(mp[inw]) / (0.92 * np.sum(p[inw]) + 1e-24))
        if r > 1:
            for md in out:
                if lo <= md[0] < hi:
                    md[1] /= np.sqrt(r)
    return out


_TOT = {}


def band_totals(m, on):
    # (an array's id is reused once it is freed: the key holds its content too)
    key = (id(m), on, len(m), float(np.sum(np.abs(m[on:on + 4800]))))
    if key not in _TOT:
        tot = []
        for c in CENTRES:
            lo, hi = edges(c)
            tot.append(None if lo > 0.47 * SR else
                       np.minimum(at_breakpoints(band_power(m, lo, hi), on), broad_power(m, on)))
        _TOT[key] = tot
    return _TOT[key]


REGIONS = [(30, 150), (150, 1000), (1000, 2000), (2000, 4000), (4000, 8000), (8000, 12000), (12000, 18000)]


def tonal_frac(m, on, a, b):
    """Per region, the share of the window's power in bins 10 dB over the local median."""
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w)
    P = np.abs(np.fft.rfft(w * np.hanning(n), 4 * n)) ** 2
    f = np.fft.rfftfreq(4 * n, 1 / SR)
    d = 10 * np.log10(P + 1e-30)
    e = d - ndimage.median_filter(d, size=int(300 / f[1]) | 1)
    out = []
    for lo, hi in REGIONS:
        s = (f >= lo) & (f < hi)
        out.append(float(np.clip(P[s][e[s] > 10].sum() / (P[s].sum() + 1e-30), 0.01, 0.97)))
    return np.array(out)


def noise_env(totals, modes, t, resid=None):
    """Each band's noise: what its partials leave of its power — and never more than the
    recording's own noise there (`resid`, the residual's share: drums-gen 2), so a tail the
    partials under-read is not made up as hiss."""
    env = []
    for k, (c, p) in enumerate(zip(CENTRES, totals)):
        lo, hi = edges(c)
        if p is None:
            env.append([-200.0] * len(BP_MS))
            continue
        q = np.maximum(p - modal_power(modes, lo, hi, t), 0.03 * p)
        if resid is not None:
            q = np.minimum(q, np.maximum(resid[k] * p, 1e-3 * p))
        env.append([float(x) for x in 10 * np.log10(q + 1e-24)])
    return env


# From here a cymbal's noise is read off the residual (drums-gen 2), s.
RESID_FROM = 0.1


def residual_share(m, on, drum=False):
    """Per band and breakpoint, the share of the recording's power that is noise: the
    spectrum's local median (a noise's median power is ln 2 of its mean) against its whole,
    bin by bin no more than the bin — the residual once the partials standing over the
    median are taken out (drums-gen 2: the bands had been fitted to the band's whole power,
    partials included, and a cymbal's tail turned to hiss). 1 before `RESID_FROM` — but a
    drum's from its strike: its first 85 ms read in one window from the onset (a tom's
    fundamental held as noise in its bands leaked 20-55 Hz into the stroke, cue 4)."""
    out = np.ones((NB, len(BP_MS)))
    for j, tb in enumerate(BP_MS):
        t = tb / 1000
        if t < RESID_FROM and not drum:
            continue
        n = 4096 if t < 0.5 else 16384
        a = on + int(t * SR) - n // 2
        if drum and t < RESID_FROM:
            a = on
        w = m[max(a, 0):max(a, 0) + n]
        if len(w) < n:
            continue
        P = np.abs(np.fft.rfft(w * np.hanning(n))) ** 2
        f = np.fft.rfftfreq(n, 1 / SR)
        d = 10 * np.log10(P + 1e-30)
        # (the 10th percentile: the floor between the partials; a noise's is 0.105 of its mean)
        med = 10 ** (ndimage.percentile_filter(d, 10, size=int((150 if drum else 300) / f[1]) | 1) / 10) / 0.1054
        noise = np.minimum(med, P)
        for k, c in enumerate(CENTRES):
            lo, hi = edges(c)
            s = (f >= lo) & (f < hi)
            if s.sum() >= 3 and P[s].sum() > 0:
                out[k, j] = float(np.clip(noise[s].sum() / P[s].sum(), 0.0, 1.0))
    return out


def defloor(p):
    """A band's tail past where it reaches the recording's floor (the room, the other
    mics' hiss) goes on falling at the slope it had: the floor is not the cymbal's."""
    t = np.array(BP_MS) / 1000
    d = 10 * np.log10(p + 1e-30)
    floor = d[-3:].min()
    i = int(np.argmax(d))
    k = next((j for j in range(i + 1, len(d)) if d[j] < floor + 10 and t[j] > 0.2), None)
    if k is None or k - i < 2 or t[-1] <= t[k]:
        return p
    a = max(i, k - 3)
    slope = (d[k] - d[a]) / (t[k] - t[a])
    after = (d[-1] - d[k]) / (t[-1] - t[k])
    # A floor is flat (under 4 dB/s) and far under the band's peak; a slow tail is the
    # cymbal's own.
    if slope >= 0 or after < 0.5 * slope or after < -4.0 or d[i] - floor < 35.0:
        return p
    d[k:] = d[k] + slope * (t[k:] - t[k])
    return 10 ** (d / 10)


def shares(y, on, piece_win, freqs=None, n=6, ref=1000):
    """The strongest partials above 1 kHz and each one's share of the energy there
    (analysis-drums s11's way): `(hz, share)`; or the shares at `freqs`."""
    a, b = piece_win
    w = y[on + int(a * SR):on + int(b * SR)]
    k = len(w)
    P = np.abs(np.fft.rfft(w * np.hanning(k), 4 * k)) ** 2
    f = np.fft.rfftfreq(4 * k, 1 / SR)
    hi = f > ref
    tot = P[hi].sum()
    if freqs is None:
        d = 10 * np.log10(P + 1e-30)
        pk, _ = signal.find_peaks(d * hi, distance=max(1, int(30 / f[1])))
        pk = pk[np.argsort(d[pk])[::-1]][:n]
    else:
        pk = [int(np.argmax(np.where(np.abs(f - z) < 3, P, 0))) for z in freqs]
    return [(float(f[i]), float(P[max(0, i - 8):i + 9].sum() / tot)) for i in pk]


def match_strongest(m, on, spec, modes, totals, t, vel, resid=None):
    """The recording's strongest partials keep their share: each of the six strongest
    above 1 kHz (a bell's two or three tones; an open hat's few) brought to the share
    it has in the recording — both ways, a partial too strong is cut."""
    import synth
    if not modes:
        return modes
    win = (0.01, 0.12) if spec["win"][1] < 0.3 else (0.1, 1.1)
    ref = shares(m, on, win, ref=spec.get("ref", 1000))
    pad = int(0.02 * SR)
    for it in range(3):
        env = noise_env(totals, modes, t, resid)
        tab = dict(centres=CENTRES, bp_ms=BP_MS,
                   pieces=dict(x=[dict(env=env, modes=modes, vel=vel, stick=[0.0, 0.0])]))
        y = np.zeros(pad + int((win[1] + 0.1) * SR))
        y[pad:] = synth.render(tab, "x", vel, len(y) - pad, np.random.default_rng(5))
        got = shares(y, pad, win, [hz for hz, _ in ref], ref=spec.get("ref", 1000))
        # and our own strongest that the recording does not have so strong
        ours = shares(y, pad, win, ref=spec.get("ref", 1000))
        want = {hz: sh for hz, sh in ref}
        # (once, and only what stands out twice as much as there: a measured partial is
        # broader than a pure one, so its narrow share reads low)
        for hz, sh in ours if it == 0 else []:
            if all(abs(hz - r) > 3 for r in want):
                rs = shares(m, on, win, [hz], ref=spec.get("ref", 1000))[0][1]
                if sh > 2.0 * rs:
                    want[hz] = 2.0 * rs
                    got.append((hz, sh))
        for (hz, g), r in zip(got, [want.get(h, None) for h, _ in got]):
            if r is None or g <= 0:
                continue
            j = int(np.argmin([abs(md[0] - hz) for md in modes]))
            if abs(modes[j][0] - hz) > 3:
                continue
            modes[j][1] *= float(np.clip(np.sqrt(r / g), 0.5, 4.0))
    return modes


def schroeder(p, lo_db=-5.0, hi_db=-25.0):
    """A band's T60 from its power (Schroeder's backward integral, -5 to -25 dB), s."""
    s = np.cumsum(p[::-1])[::-1]
    s = 10 * np.log10(s / (s[0] + 1e-30) + 1e-30)
    i0, i1 = int(np.argmax(s <= lo_db)), int(np.argmax(s <= hi_db))
    if i1 <= i0 + 10:
        return 1.0
    tt = np.arange(len(s)) / SR
    sl = np.polyfit(tt[i0:i1], s[i0:i1], 1)[0]
    return float(np.clip(-60.0 / sl, 0.05, 5.0)) if sl < 0 else 1.0


def broad_power(m, on):
    """The recording's whole power at the breakpoints, no filter: no band may hold more
    (a narrow low band's filter smears a short sound's energy into a tail it never had)."""
    return at_breakpoints(m * m, on)


def fit_layer(m, on, spec, vel=100):
    import synth
    t = np.array(BP_MS) / 1000
    broad = broad_power(m, on)
    totals = []
    for c in CENTRES:
        lo, hi = edges(c)
        totals.append(None if lo > 0.47 * SR else
                      defloor(np.minimum(at_breakpoints(band_power(m, lo, hi), on), broad)))
    # A cymbal's noise is the residual's (drums-gen 2); a drum's, its band less its partials.
    resid = residual_share(m, on, drum=spec.get("ref", 1000) == 30) if RESID else None
    drum = spec.get("ref", 1000) == 30
    modes = assign_rise(find_modes(m, on, spec), totals)
    if drum and FLOOR_T60:
        # A drum's upper modes ring no longer than the recording's 1-4 kHz does (cue 5:
        # the toms' rang 1-9 s against 0.35-0.45; what stood there late was other drums).
        hi_t60 = schroeder(band_power(m, 1000, 4000)[on:on + int(3 * SR)])
        modes = [[hz, a, min(t, max(0.12, 1.2 * hi_t60)) if hz >= 1000 else t] + md
                 for hz, a, t, *md in modes]
    modes = cap_modes(clamp_t60(modes, totals, spec["win"][1], resid, drum), m, on, spec)
    # The estimator under-reads a measured partial (its lobe is wider than a pure
    # sinusoid's); closed loop: the tonal layer's level per region until the synthesis's
    # share of tonal power matches the recording's.
    a, b = spec["win"]
    want = tonal_frac(m, on, a, b)
    pad = int(0.02 * SR)
    base = modes
    lo_g = np.full(len(REGIONS), -0.5)
    hi_g = np.full(len(REGIONS), 1.2)
    # the first try: as measured (log gain 0), then bisect
    first = True
    region = [next((i for i, (lo, hi) in enumerate(REGIONS) if lo <= hz < hi), None)
              for hz, *_ in base]

    def scaled(lg):
        return cap_modes(clamp_t60([[md[0], md[1] * (10 ** lg[k] if k is not None else 1.0)]
                                    + md[2:] for md, k in zip(base, region)], totals, spec["win"][1], resid, drum), m, on, spec)
    for _ in range(8):
        mid = np.zeros(len(REGIONS)) if first else 0.5 * (lo_g + hi_g)
        first = False
        modes = scaled(mid)
        env = noise_env(totals, modes, t, resid)
        tab = dict(centres=CENTRES, bp_ms=BP_MS,
                   pieces=dict(x=[dict(env=env, modes=modes, vel=vel, stick=[0.0, 0.0])]))
        y = np.zeros(pad + int((b + 0.1) * SR))
        y[pad:] = synth.render(tab, "x", vel, len(y) - pad, np.random.default_rng(3))
        got = tonal_frac(y, pad, a, b)
        if DEBUG:
            print(vel, np.round(want, 2), np.round(got, 2), np.round(mid, 2))
        # Close enough is kept: a region already as tonal as the recording (a bell's,
        # near saturation) is not pushed further — that would raise its weak partials
        # up to its strong ones.
        near = np.abs(got - want) <= 0.06
        lo_g = np.where(near, mid, np.where(got < want, mid, lo_g))
        hi_g = np.where(near, mid, np.where(got < want, hi_g, mid))
    modes = scaled(0.5 * (lo_g + hi_g))
    modes = match_strongest(m, on, spec, modes, totals, t, vel, resid)
    env = noise_env(totals, modes, t, resid)
    env = np.array(env)
    # The energy below 450 Hz over the first 2 s, held at the target.
    tw = np.diff(np.concatenate([t, [t[-1] * 1.4]]))
    lowk = np.array([c < 450 for c in CENTRES])
    e_band = (10 ** (env / 10) * tw).sum(1)
    low, tot = e_band[lowk].sum(), e_band.sum()
    want = 10 ** (spec.get("low_db", 0.0) / 10)
    if "low_db" in spec and low / tot > want:
        g = want * (tot - low) / (low * (1 - want))
        env[lowk] += 10 * np.log10(g)
    return dict(env=env.round(2).tolist(), modes=[[round(x, 6) for x in md] for md in modes])


def first3(y, on, lo, hi):
    """Energy in a band in the first 3 ms."""
    sos = signal.butter(4, [lo, hi], btype="band", fs=SR, output="sos")
    z = signal.sosfiltfilt(sos, y)
    return float((z[on:on + 144] ** 2).sum())


def e3(y, on):
    return np.array([first3(y, on, 200, 1500), first3(y, on, 6000, 20000)])


def fit_stick(m, on, lay, vel, foot=False):
    """The stick's contact: a broadband burst and a dull knock, their levels what the
    first 3 ms lack without them (the 0.2-1.5 kHz and 6-20 kHz energies). The foot
    (the pedal's chick) has no stick, only the knock of the plates."""
    import synth
    from scipy.optimize import nnls
    rng = np.random.default_rng(7)
    tab = dict(centres=CENTRES, bp_ms=BP_MS, pieces=dict(x=[dict(lay, vel=vel, stick=[0.0, 0.0])]))
    n = SR // 4
    pad = int(0.02 * SR)

    def put(sig):
        y = np.zeros(n + pad)
        y[pad:] = sig
        return y
    want = e3(m, on) - e3(put(synth.render(tab, "x", vel, n, rng)), pad)
    a_ = np.zeros((2, 2))
    for j in range(2):
        g = [0.0, 0.0]
        g[j] = 1.0
        a_[:, j] = np.mean([e3(put(synth.stick(g, vel, n, np.random.default_rng(k))), pad)
                            for k in range(6)], axis=0)
    if foot:
        a_[:, 0] = 0
    x, _ = nnls(a_, np.maximum(want, 0))
    return [float(np.sqrt(x[0])), float(np.sqrt(x[1]))]


def level_db(y, on):
    return 10 * np.log10(np.mean(y[on:on + SR // 2] ** 2) + 1e-30)


def high_db(y, on):
    """The 500-ms RMS above 450 Hz (below, the recording is mostly the other drums)."""
    sos = signal.butter(4, 450, btype="high", fs=SR, output="sos")
    return level_db(signal.sosfiltfilt(sos, y), on)


def fit_level(m, on, lay, vel, spec=None):
    """The whole layer's level against the recording's (500-ms RMS above 450 Hz: what
    the low end gave up to the bleed is not made up by the rest)."""
    import synth
    tab = dict(centres=CENTRES, bp_ms=BP_MS, pieces=dict(x=[dict(lay, vel=vel)]))
    pad = int(0.02 * SR)
    d = []
    for k in range(3):
        y = np.zeros(pad + SR)
        y[pad:] = synth.render(tab, "x", vel, SR, np.random.default_rng(11 + k))
        if spec is not None and spec.get("level_hp", 450) == 0:
            d.append(level_db(m, on) - level_db(y, pad))
        else:
            d.append(high_db(m, on) - high_db(y, pad))
    g = float(np.mean(d))
    lay["env"] = (np.array(lay["env"]) + g).round(2).tolist()
    lay["modes"] = [[md[0], md[1] * 10 ** (g / 20)] + list(md[2:]) for md in lay["modes"]]
    lay["stick"] = [x * 10 ** (g / 20) for x in lay["stick"]]
    return lay


def low_peak(m, on, a, b, lo=30, hi=400):
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w)
    P = np.abs(np.fft.rfft(w * np.hanning(n), 16 * n)) ** 2
    f = np.fft.rfftfreq(16 * n, 1 / SR)
    k = (f >= lo) & (f <= hi)
    return float(f[k][np.argmax(P[k])])


def fit_glide(m, on):
    """A drum's pitch glide: its strongest low partial early (10-60 ms) against late
    (150-450 ms), as a ratio's excess falling with a time constant fitted from between
    (60-150 ms): `[excess, tau s]`."""
    late = low_peak(m, on, 0.15, 0.45)
    early = low_peak(m, on, 0.01, 0.06, late * 0.9, late * 1.3)
    mid = low_peak(m, on, 0.06, 0.15, late * 0.9, late * 1.3)
    g0 = max(early / late - 1.0, 0.0)
    gm = max(mid / late - 1.0, 0.0)
    if g0 < 1e-3:
        return [0.0, 0.05]
    # g(t) = g0 exp(-(t - 0.035)/tau), seen at t = 0.105 as gm
    tau = 0.07 / np.log(g0 / gm) if 0 < gm < g0 else 0.08
    tau = float(np.clip(tau, 0.02, 0.4))
    # carried back from the early window's centre to the strike
    return [float(min(g0 * np.exp(0.035 / tau), 0.3)), tau]


# A drum's early noise under its fundamental cut (drums-gen 2; `TYMP_FIT_SUBCUT=0` off).
SUBCUT = __import__("os").environ.get("TYMP_FIT_SUBCUT", "1") != "0"


def subcut(lay, k, db=20.0, until_ms=150.0):
    """Cue 4, the sub-sonic thump: a quarter-octave of noise at a drum's fundamental is a
    broad low push, not the partial the recording has there — its skirts put 20-55 Hz into
    the stroke (a tom 10-35 dB over the recordings' -35...-45 dB re the peak in the first
    80 ms). The bands under `k` times the fundamental (the lowest partial within 10 dB of
    the strongest) give up `db` over the first `until_ms`; the partials carry the push."""
    top = max(md[1] for md in lay["modes"])
    f0 = min(md[0] for md in lay["modes"] if md[1] > top * 10 ** (-10 / 20))
    for b, c in enumerate(CENTRES):
        if c < k * f0:
            lay["env"][b] = [x - db if t < until_ms and x > -150 else x
                              for x, t in zip(lay["env"][b], BP_MS)]


def fit(path, pieces):
    x = load(path)
    out = dict(centres=CENTRES, bp_ms=BP_MS, pieces={})
    for p in pieces:
        spec = SPEC[p]
        layers = []
        for v in VELS:
            m, on = hit(x, p, v)
            lay = fit_layer(m, on, spec, v)
            lay["vel"] = v
            lay["stick"] = fit_stick(m, on, lay, v, foot=(p == "hat_pedal"))
            lay = fit_level(m, on, lay, v, spec)
            if spec.get("subcut") and SUBCUT:
                subcut(lay, spec["subcut"])
            if spec.get("glide"):
                lay["glide"] = fit_glide(m, on)
            lay["level"] = float(level_db(m, on))
            layers.append(lay)
        if spec.get("glide"):
            # One law for the piece: the head's tension grows with the strike's energy, so
            # the glide with the velocity squared (Avanzini, Marogna & Bank), its largest
            # the layers' (no more than the piece allows) and one time constant, theirs.
            top = min(max(l["glide"][0] for l in layers), spec["glide"])
            tau = float(np.clip(np.median([l["glide"][1] for l in layers]), 0.03, 0.2))
            for lay in layers:
                lay["glide"] = [top * (lay["vel"] / 127.0) ** 2, tau]
        out["pieces"][p] = layers
    return out


if __name__ == "__main__":
    pieces = sys.argv[3:] or ["crash", "hat_closed", "hat_open", "hat_pedal"]
    json.dump(fit(sys.argv[1], pieces), open(sys.argv[2], "w"))
