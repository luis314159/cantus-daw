"""Loudness against outside values: ITU-R BS.1770-4's published K-weighting coefficients at
48 kHz (Tables 1 and 2), its one-channel 1 kHz rule, the expected readings of EBU Tech 3341
(v4, 2016) cases 1-5 and Tech 3342 (v4) cases 1-4 for the signals those documents define,
a true peak between samples; and, when present, the EBU loudness test set's own files
(`SOUNDLAB_EBU`, a folder of `seq-3341-*.wav`: tech.ebu.ch serves them behind a browser
challenge, so they are fetched by hand) and the kits' Rust meter on a generated kit
(`SOUNDLAB_KIT_KEY`, a kit's key folder with `levels.csv`)."""
import csv
import os
import unittest
from pathlib import Path

import numpy as np

from common import lab  # noqa: F401
from soundlab import loudness as L, audio
from synth import sine_segments, SR


class Coefficients(unittest.TestCase):
    def test_bs1770_tables_1_and_2_at_48k(self):
        (b1, a1), (b2, a2) = L.kweight_coeffs(48000)
        # BS.1770-4 Table 1 (the pre-filter) and Table 2 (the RLB high-pass), as published.
        for got, want in zip(b1 + a1[1:], [1.53512485958697, -2.69169618940638, 1.19839281085285,
                                           -1.69065929318241, 0.73248077421585]):
            self.assertAlmostEqual(got, want, places=10)
        for got, want in zip(b2 + a2[1:], [1.0, -2.0, 1.0, -1.99004745483398, 0.99007225036621]):
            self.assertAlmostEqual(got, want, places=10)


class Tech3341(unittest.TestCase):
    def check(self, segs, want, tol=0.1, what=""):
        x = sine_segments(segs)
        got = L.measure(x, SR)
        self.assertLessEqual(abs(got["integrated_lufs"] - want), tol, f"{what}: {got['integrated_lufs']:.3f}")
        return got

    def test_cases_1_and_2(self):
        for lev in (-23.0, -33.0):
            g = self.check([(lev, 20.0)], lev, what=f"case {lev}")
            self.assertLessEqual(abs(g["momentary_max_lufs"] - lev), 0.1)
            self.assertLessEqual(abs(g["short_term_max_lufs"] - lev), 0.1)

    def test_case_3_relative_gate(self):
        self.check([(-36, 10), (-23, 60), (-36, 10)], -23.0, what="case 3")

    def test_case_4_absolute_gate(self):
        self.check([(-72, 10), (-36, 10), (-23, 60), (-36, 10), (-72, 10)], -23.0, what="case 4")

    def test_case_5_energy_mean(self):
        self.check([(-26, 20), (-20, 20.1), (-26, 20)], -23.0, what="case 5")

    def test_one_channel_full_scale_sine_reads_minus_3_01(self):
        # BS.1770-4 §2: a 0 dBFS 1 kHz sine on one channel reads -3.01 LKFS.
        x = sine_segments([(0.0, 10.0)], channels=1)
        self.assertAlmostEqual(L.lufs(x, SR), -3.01, delta=0.05)

    def test_dual_mono_reads_3_01_more(self):
        x = sine_segments([(-20.0, 10.0)], channels=1)
        self.assertAlmostEqual(L.lufs(np.repeat(x, 2, axis=1), SR) - L.lufs(x, SR), 3.01, delta=0.01)


class Tech3342(unittest.TestCase):
    def test_cases_1_to_4(self):
        for segs, want in [([(-20, 20), (-30, 20)], 10), ([(-20, 20), (-15, 20)], 5),
                           ([(-40, 20), (-20, 20)], 20),
                           ([(-50, 20), (-35, 20), (-20, 20), (-35, 20), (-50, 20)], 15)]:
            got = L.measure(sine_segments(segs), SR)["lra_lu"]
            self.assertLessEqual(abs(got - want), 1.0, f"{segs}: LRA {got:.2f}")


class TruePeak(unittest.TestCase):
    def test_a_sine_at_fs4_between_samples(self):
        # A 0 dBFS sine at fs/4 sampled 45 degrees off its peaks: every sample at -3.01 dBFS,
        # the true peak 0 dBTP (Tech 3341's tolerance: +0.2 / -0.4 dB).
        n = np.arange(SR)
        x = np.sin(2 * np.pi * n / 4 + np.pi / 4)[:, None]
        r = L.measure(x, SR)
        self.assertAlmostEqual(r["sample_peak_dbfs"], -3.01, delta=0.01)
        self.assertTrue(-0.4 <= r["true_peak_dbtp"] <= 0.2, r["true_peak_dbtp"])


EBU = {"seq-3341-1-16bit.wav": -23.0, "seq-3341-2-16bit.wav": -33.0, "seq-3341-3-16bit-v02.wav": -23.0,
       "seq-3341-4-16bit-v02.wav": -23.0, "seq-3341-5-16bit-v02.wav": -23.0}


@unittest.skipUnless(os.environ.get("SOUNDLAB_EBU"), "SOUNDLAB_EBU (the EBU loudness test set) not set")
class EbuFiles(unittest.TestCase):
    def test_the_published_files(self):
        root = Path(os.environ["SOUNDLAB_EBU"])
        seen = 0
        for name, want in EBU.items():
            hits = list(root.rglob(name))
            if not hits:
                continue
            x, sr = audio.read(hits[0])
            self.assertLessEqual(abs(L.lufs(x, sr) - want), 0.1, name)
            seen += 1
        self.assertGreater(seen, 0, "no EBU file found")


@unittest.skipUnless(os.environ.get("SOUNDLAB_KIT_KEY") and os.environ.get("SOUNDLAB_LIGHT") != "1",
                     "SOUNDLAB_KIT_KEY (a generated kit's key folder) not set")
class AgainstTheKitsRustMeter(unittest.TestCase):
    def test_lufs_after_the_match(self):
        """The Rust generators match with cantus-dsp's LoudnessMeter (itself held to Tech 3341)
        and write each file's reading in levels.csv: this BS.1770 reads the same files the
        same, to 0.05 LU."""
        key = Path(os.environ["SOUNDLAB_KIT_KEY"])
        kit = Path(str(key)[: -len("-key")]) / "audio"
        rows = list(csv.DictReader(open(key / "levels.csv", encoding="utf-8")))
        worst, n = 0.0, 0
        for row in rows[:: max(len(rows) // 8, 1)]:
            p = kit / row["file"]
            if not p.exists():
                continue
            x, sr = audio.read(p)
            worst = max(worst, abs(L.lufs(x, sr) - float(row["lufs_after"])))
            n += 1
        self.assertGreater(n, 0)
        self.assertLess(worst, 0.05, f"worst {worst:.3f} LU over {n} files")


if __name__ == "__main__":
    unittest.main()
