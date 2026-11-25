"""The features on synthetic notes built with known values (`synth.py`), and on one published
dataset's labels: TinySOL's metadata names every file's pitch (Cella, Ghisi, Lostanlen et al.,
Zenodo 3685367, CC BY 4.0) — YIN must agree with it (SOUNDLAB_REFS, the downloads folder)."""
import csv
import os
import unittest
from pathlib import Path

import numpy as np

from common import lab  # noqa: F401
from soundlab import audio
from soundlab.features import Note, analyse, centroid_track, band_t60, schroeder, _decay_time
from synth import harmonic_tone, pluck, decaying_noise, midi_hz, SR

RANGE = (40.0, 1700.0)


def sus(x, nominal=None, ch=None):
    return Note(x[:, None] if ch is None else ch, SR, "sustained", RANGE, nominal)


def dec(x, nominal=None):
    return Note(x[:, None], SR, "decaying", RANGE, nominal)


class Pitch(unittest.TestCase):
    def test_f0_of_sustained_tones(self):
        for f0 in (41.2, 98.0, 196.0, 440.0, 880.0):
            x, _ = harmonic_tone(f0)
            r = analyse(sus(x, nominal=69 + 12 * np.log2(f0 / 440.0)), ["pitch"])["pitch"]
            self.assertLess(abs(1200 * np.log2(r["f0_hz"] / f0)), 1.0, f0)
            self.assertLess(abs(r["cents_from_nominal"]), 1.0, f0)
            self.assertLess(r["spread_cents_p10_p90"], 2.0, f0)

    def test_f0_of_a_stretched_pluck(self):
        x, _ = pluck(65.4, B=1.2e-4)
        r = analyse(dec(x), ["pitch"])["pitch"]
        self.assertLess(abs(1200 * np.log2(r["f0_hz"] / 65.4)), 0.5)

    def test_vibrato_spread(self):
        # +-18 cents of sinusoid: p10-p90 of its values is 2 * 18 * sin(0.4 pi) = 34.2 cents.
        x, _ = harmonic_tone(196.0, dur=3.5, vib_rate=5.8, vib_cents=18.0)
        r = analyse(sus(x), ["pitch"])["pitch"]
        self.assertAlmostEqual(r["spread_cents_p10_p90"], 2 * 18 * np.sin(0.4 * np.pi), delta=2.0)


class Harmonics(unittest.TestCase):
    def test_sustained_partials(self):
        for f0 in (98.0, 440.0):
            x, tr = harmonic_tone(f0)
            r = analyse(sus(x), ["harmonics"])["harmonics"]
            want = tr["amps_db"][:20]
            got = np.array(r["h_db_re_f1"][:len(want)], float)
            ok = np.isfinite(got)
            self.assertLess(np.max(np.abs(got[ok] - want[: len(got)][ok])), 0.5, f0)

    def test_pluck_peaks_and_inharmonicity(self):
        # The peaks are read under an 8-period window: at 41 Hz (194 ms) a fast-decaying
        # partial's peak reads up to 2.3 dB low (the window averages its fall), so 98 Hz up.
        for f0, B in ((41.2, 4e-5), (98.0, 1e-4), (196.0, 6e-5)):
            x, tr = pluck(f0, B=B)
            r = analyse(dec(x), ["harmonics", "inharmonicity"])
            self.assertLess(abs(r["inharmonicity"]["B"] / B - 1), 0.02, (f0, B))
            if f0 < 90:
                continue
            h = np.array([v if v is not None else np.nan for v in r["harmonics"]["h_db_re_max"][:8]])
            want = -20 * np.log10(np.arange(1, 9))
            self.assertLess(np.nanmax(np.abs(h - want)), 1.0, (f0, h))

    def test_sustained_harmonic_tone_is_harmonic(self):
        x, _ = harmonic_tone(196.0)
        r = analyse(sus(x), ["inharmonicity"])["inharmonicity"]
        self.assertLess(r["B"], 2e-6)
        self.assertLess(r["mode_lock_dev_cents_median"], 0.1)


class Centroid(unittest.TestCase):
    def test_harmonic_centroid(self):
        x, tr = harmonic_tone(220.0)
        p = 10 ** (tr["amps_db"] / 10)
        f = 220.0 * np.arange(1, 21)
        want = np.sum(f * p) / np.sum(p)
        r = analyse(sus(x), ["centroid"])["centroid"]
        self.assertLess(abs(r["harmonic_centroid_hz"] / want - 1), 0.01)
        self.assertLess(abs(r["mean_hz"] / want - 1), 0.03)

    def test_a_darkening_pluck(self):
        # Two partials, 200 and 2000 Hz, equal at the onset, the upper falling 60 dB/s faster:
        # the centroid at t is (200 + 2000 g) / (1 + g), g = 10^(-6 t).
        n = int(2.0 * SR)
        t = np.arange(n) / SR
        x = np.sin(2 * np.pi * 200 * t) + 10 ** (-3 * t) * np.sin(2 * np.pi * 2000 * t)
        x = np.concatenate([np.zeros(int(0.2 * SR)), 0.3 * x])
        tt, c, _ = centroid_track(x, SR, nfft=1024, hop=128)
        for at in (0.1, 0.3, 0.6):
            g = 10 ** (-6 * at)
            want = (200 + 2000 * g) / (1 + g)
            got = np.interp(0.2 + at, tt, c)
            self.assertLess(abs(got / want - 1), 0.05, (at, got, want))


class Noise(unittest.TestCase):
    def test_sustained_noise_level(self):
        # White noise 30 dB under the harmonics: in the valid bands the residual reads it within
        # the study's measured bias (+0.4 ... +1.1 dB), held here to 1.5 dB.
        x, tr = harmonic_tone(196.0, noise_db=-30.0)
        r = analyse(sus(x), ["noise"])["noise"]
        fc = np.array(r["bands_hz"])
        valid = (fc >= 300) & (fc >= 5 * 196) & (fc <= 0.42 * SR)
        want = -30.0 + 10 * np.log10(self._share(fc[valid]))
        self.assertLess(abs(r["res_valid_db_re_harm"] - want), 1.5, (r["res_valid_db_re_harm"], want))

    @staticmethod
    def _share(fc):
        # White noise: each third octave holds its bandwidth's share of 0..fs/2.
        bw = fc * (2 ** (1 / 6) - 2 ** (-1 / 6))
        return bw.sum() / (SR / 2)

    def test_decaying_between_partials(self):
        clean, _ = pluck(196.0, B=0.0)
        noisy, _ = pluck(196.0, B=0.0, noise_db=-20.0)
        a = analyse(dec(clean), ["noise"])["noise"]["hnr_valid_median_db"]
        b = analyse(dec(noisy), ["noise"])["noise"]["hnr_valid_median_db"]
        self.assertGreater(a - b, 15.0, (a, b))


class Attack(unittest.TestCase):
    def test_rise_10_90(self):
        # 1 - exp(-t / tau): 10 % at tau ln(10/9), 90 % at tau ln 10 -> tau ln 9.
        for tau in (0.01, 0.03, 0.08):
            x, _ = harmonic_tone(220.0, attack_tau=tau, nh=1)
            r = analyse(sus(x), ["attack"])["attack"]
            self.assertLess(abs(r["rise_10_90_ms"] - 1000 * tau * np.log(9)), 3.0 + 0.1 * 1000 * tau * np.log(9), tau)
            self.assertLess(abs(r["onset_s"] - 0.3), 0.01)

    def test_pluck_peak_and_burst(self):
        x, _ = pluck(98.0)
        r = analyse(dec(x), ["attack"])["attack"]
        self.assertLess(r["t_peak_ms"], 15.0)
        self.assertLess(abs(r["onset_s"] - 0.2), 0.005)


class Decay(unittest.TestCase):
    def test_partial_t60(self):
        x, tr = pluck(98.0, B=1e-4)
        r = analyse(dec(x), ["decay"])["decay"]
        got = np.array([v if v else np.nan for v in r["t60_s"][:12]])
        want = np.array(tr["t60"][:12])
        self.assertLess(np.nanmax(np.abs(got / want - 1)), 0.05)

    def test_release(self):
        x, _ = harmonic_tone(220.0, release_db_s=40.0, dur=4.8, t_off=3.0)
        r = analyse(sus(x), ["decay"])["decay"]
        self.assertAlmostEqual(r["release_db_per_s"], 40.0, delta=1.0)

    def test_band_t60_of_decaying_noise(self):
        for t60 in (0.4, 1.0, 2.5):
            y = decaying_noise(t60, dur=max(3.0, 1.5 * t60 + 0.5))
            self.assertLess(abs(band_t60(y, SR, 0.2) / t60 - 1), 0.08, t60)

    def test_room_t20(self):
        y = decaying_noise(1.2, dur=3.0)
        t, d = schroeder(y, SR, 0.2)
        self.assertLess(abs(_decay_time(t, d, -5.0, -25.0) / 1.2 - 1), 0.05)
        self.assertLess(abs(_decay_time(t, d, 0.0, -10.0) / 1.2 - 1), 0.05)


class Vibrato(unittest.TestCase):
    def test_rate_extent_and_regular_cycles(self):
        for f0 in (98.0, 440.0):
            x, _ = harmonic_tone(f0, dur=4.0, vib_rate=5.5, vib_cents=20.0)
            r = analyse(sus(x), ["vibrato"])["vibrato"]
            self.assertAlmostEqual(r["rate_hz"], 5.5, delta=0.05)
            self.assertAlmostEqual(r["extent_cents"], 20.0, delta=1.0)
            self.assertAlmostEqual(r["extent_pp_cents"], 40.0, delta=2.5)
            self.assertLess(r["rate_jitter"], 0.06)
            self.assertLess(r["extent_jitter"], 0.08)
            self.assertEqual(r["present"], 1.0)

    def test_no_vibrato(self):
        x, _ = harmonic_tone(196.0)
        r = analyse(sus(x), ["vibrato"]).get("vibrato", {})
        self.assertEqual(r.get("present", 0.0), 0.0)


class Stereo(unittest.TestCase):
    def test_balance_width_correlation(self):
        x, _ = harmonic_tone(220.0)
        r = analyse(Note(np.stack([x, 0.5 * x], 1), SR, "sustained", RANGE), ["stereo"])["stereo"]
        self.assertAlmostEqual(r["balance_lr_db"], 6.02, delta=0.01)
        self.assertAlmostEqual(r["side_mid_db"], 20 * np.log10(0.25 / 0.75), delta=0.01)
        self.assertAlmostEqual(r["correlation"], 1.0, delta=1e-6)
        rng = np.random.default_rng(5)
        w = 0.003 * rng.standard_normal((len(x), 2))
        r = analyse(Note(np.stack([x, x], 1) + w, SR, "sustained", RANGE), ["stereo"])["stereo"]
        self.assertGreater(r["correlation"], 0.9)
        r = analyse(Note(np.stack([x, -x], 1), SR, "sustained", RANGE), ["stereo"])["stereo"]
        self.assertAlmostEqual(r["correlation"], -1.0, delta=1e-6)


@unittest.skipUnless(os.environ.get("SOUNDLAB_REFS") and os.environ.get("SOUNDLAB_LIGHT") != "1",
                     "SOUNDLAB_REFS (the downloads folder) not set")
class TinySolPitches(unittest.TestCase):
    def test_yin_agrees_with_the_published_pitch(self):
        """TinySOL's metadata names each file's pitch: the lab's f0 lies within 50 cents of it
        (the orchestra tunes near A = 440-442 Hz) on a spread of cello and bass notes."""
        root = Path(os.environ["SOUNDLAB_REFS"]) / "arcus" / "tinysol"
        meta = root / "TinySOL_metadata.csv"
        if not meta.exists():
            self.skipTest("no TinySOL here")
        rows = [r for r in csv.DictReader(open(meta, encoding="utf-8"))
                if r["Instrument (abbr.)"] in ("Vc", "Cb") and r["Dynamics"] == "mf"]
        names = "C C# D D# E F F# G G# A A# B".split()
        hinted, free, n = [], [], 0
        for r in rows[::6]:
            p = next(iter(root.rglob(Path(r["Path"]).name)), None)
            if p is None:
                continue
            pitch = r["Pitch"]
            m = 12 * (int(pitch[-1]) + 1) + names.index(pitch[:-1])
            x, sr = audio.read(p, channels=1)
            fam = (60, 1100) if r["Instrument (abbr.)"] == "Vc" else (28, 520)
            a = analyse(Note(x, sr, "sustained", fam, nominal=m), ["pitch"]).get("pitch")
            b = analyse(Note(x, sr, "sustained", fam, nominal=m, hint=False), ["pitch"]).get("pitch")
            if a and b:
                hinted.append(a["cents_from_nominal"])
                free.append(b["cents_from_nominal"])
                n += 1
        self.assertGreaterEqual(n, 10)
        # With the note known, every file within 50 cents; over the family's whole range, the
        # pitch class still agrees and at most one in ten is an octave off.
        self.assertLess(np.max(np.abs(hinted)), 50.0, hinted)
        cls = [((c + 600) % 1200) - 600 for c in free]
        octaves = sum(abs(c) > 600 for c in free)
        self.assertLess(np.max(np.abs(cls)), 50.0, free)
        self.assertLessEqual(octaves, n // 10, free)
        print(f"\n  TinySOL: {n} notes, cents from the published pitch: median {np.median(hinted):+.1f}, "
              f"worst {np.max(np.abs(hinted)):.1f}; over the family's range {octaves} octave error(s)")


if __name__ == "__main__":
    unittest.main()
