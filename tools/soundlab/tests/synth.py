"""Synthetic notes with known values, for the lab's tests: each generator returns the signal
and the truth it was built from. Written for these tests (not the study's `validate_synth.py`,
so the two are independent checks of the same estimators)."""
import numpy as np

SR = 48000


def sine_segments(segs, sr=SR, freq=1000.0, channels=2):
    """EBU Tech 3341/3342's signals: a sine whose level steps through (dBFS, seconds), the
    level being the sine's peak (dBFS of a full-scale sine), phase continuous."""
    parts, t0 = [], 0
    for lev, secs in segs:
        n = int(round(secs * sr))
        t = (t0 + np.arange(n)) / sr
        parts.append(10 ** (lev / 20) * np.sin(2 * np.pi * freq * t))
        t0 += n
    x = np.concatenate(parts)
    return np.repeat(x[:, None], channels, axis=1)


def harmonic_tone(f0, dur=3.0, amps_db=None, nh=20, vib_rate=0.0, vib_cents=0.0, t_on=0.3,
                  t_off=None, attack_tau=0.02, release_db_s=60.0, noise_db=None, sr=SR, seed=1,
                  vib_from=0.0):
    """A sustained note: partials 1..nh at amps_db (dB re partial 1; default -6 dB/octave with
    a ripple), a 1 - exp(-t/tau) attack, an exponential release in dB/s after t_off, an
    optional sinusoidal vibrato (rate Hz, +-cents) from vib_from s after the onset, an
    optional white noise at noise_db re the harmonic power (gated like the note)."""
    rng = np.random.default_rng(seed)
    n = int(dur * sr)
    t = np.arange(n) / sr
    t_off = dur - 0.6 if t_off is None else t_off
    ns = np.arange(1, nh + 1)
    if amps_db is None:
        amps_db = -6.0 * np.log2(ns) + 3.0 * np.sin(ns / 2.0)
    amps_db = np.asarray(amps_db, float)
    amps_db = amps_db - amps_db[0]
    tv = np.maximum(t - t_on - vib_from, 0.0)
    cents = vib_cents * np.sin(2 * np.pi * vib_rate * tv) if vib_rate else np.zeros(n)
    f = f0 * 2 ** (cents / 1200)
    phi = np.cumsum(f) / sr
    env = np.clip(1 - np.exp(-np.maximum(t - t_on, 0) / attack_tau), 0, 1) * (t >= t_on)
    env = env * np.where(t > t_off, 10 ** (-release_db_s * (t - t_off) / 20), 1.0)
    x = np.zeros(n)
    for k, a in zip(ns, amps_db):
        if k * f0 > 0.45 * sr:
            break
        x += 10 ** (a / 20) * np.sin(2 * np.pi * k * phi + rng.uniform(0, 2 * np.pi))
    x *= env
    truth = {"f0": f0, "amps_db": amps_db, "nh": nh, "t_on": t_on, "t_off": t_off}
    if noise_db is not None:
        st = (t > t_on + 0.5) & (t < t_off)
        w = rng.standard_normal(n)
        w *= np.sqrt(np.mean(x[st] ** 2) * 10 ** (noise_db / 10)) / np.std(w)
        x = x + w * env
        truth["noise_db"] = noise_db
    x += 1e-7 * rng.standard_normal(n)
    return 0.25 * x, truth


def pluck(f0, B=0.0, dur=4.0, t_on=0.2, nh=20, t60=None, sr=SR, seed=2, noise_db=None):
    """A decaying note: partials at n f0 sqrt(1 + B n^2), amplitude 1/n, each decaying with its
    own T60 (default 2.5 s / (1 + 0.15 (n - 1)))."""
    rng = np.random.default_rng(seed)
    n = int(dur * sr)
    t = np.arange(n) / sr
    x = np.zeros(n)
    t60s = []
    for k in range(1, nh + 1):
        fk = k * f0 * np.sqrt(1 + B * k * k)
        if fk > 0.45 * sr:
            break
        T = (t60[k - 1] if t60 is not None else 2.5 / (1 + 0.15 * (k - 1)))
        t60s.append(T)
        env = np.where(t >= t_on, 10 ** (-3 * (t - t_on) / T), 0.0)
        x += (1.0 / k) * env * np.sin(2 * np.pi * fk * (t - t_on) + rng.uniform(0, 2 * np.pi))
    if noise_db is not None:
        live = (t >= t_on + 0.05) & (t < t_on + 0.55)
        w = rng.standard_normal(n) * (t >= t_on)
        w *= np.sqrt(np.mean(x[live] ** 2) * 10 ** (noise_db / 10)) / np.std(w[live])
        x = x + w * np.where(t >= t_on, 10 ** (-3 * (t - t_on) / 2.5), 0.0)
    x += 1e-7 * rng.standard_normal(n)
    return 0.2 * x, {"f0": f0, "B": B, "t60": t60s, "t_on": t_on}


def decaying_noise(t60, dur=3.0, t_on=0.2, sr=SR, seed=3):
    """White noise decaying exponentially with a known T60 (a room's tail, a cymbal's band)."""
    rng = np.random.default_rng(seed)
    n = int(dur * sr)
    t = np.arange(n) / sr
    env = np.where(t >= t_on, 10 ** (-3 * (t - t_on) / t60), 0.0)
    return 0.3 * rng.standard_normal(n) * env + 1e-7 * rng.standard_normal(n)


def midi_hz(m):
    return 440.0 * 2 ** ((m - 69) / 12)
