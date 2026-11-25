"""The held-out band method on data whose answer is known, and the listening statistics
(tools/listening/abx_read.py) against published values: the one-sided binomial tail of an
ABX (12 of 16 right: p = 0.0384, the value of every ABX table), its critical counts, its
power, and BS.1534's post-screening."""
import importlib.util
import unittest

import numpy as np

from common import lab, REPO
from soundlab import band

spec = importlib.util.spec_from_file_location("abx_read", REPO / "tools" / "listening" / "abx_read.py")
abx_read = importlib.util.module_from_spec(spec)
spec.loader.exec_module(abx_read)


class Split(unittest.TestCase):
    def test_hash_rule_is_seeded_and_a_third(self):
        cells = [("arco", n, d) for n in range(36, 80) for d in ("pp", "mf", "ff")]
        a = [band.held_out(c, {"rule": "hash", "seed": 7}) for c in cells]
        b = [band.held_out(c, {"rule": "hash", "seed": 7}) for c in cells]
        c = [band.held_out(c, {"rule": "hash", "seed": 8}) for c in cells]
        self.assertEqual(a, b)
        self.assertNotEqual(a, c)
        self.assertAlmostEqual(np.mean(a), 1 / 3, delta=0.08)

    def test_arcus_mod_rule(self):
        rule = {"rule": "mod", "expr": "(note + dyn) % 3 == 0", "dyn_index": {"pp": 0, "mf": 1, "ff": 2}}
        self.assertTrue(band.held_out(("arco", 36, "pp"), rule))
        self.assertTrue(band.held_out(("arco", 38, "mf"), rule))
        self.assertFalse(band.held_out(("arco", 37, "pp"), rule))

    def test_the_rule_runs_no_code(self):
        with self.assertRaises(ValueError):
            band.held_out(("a", 1, "mf"), {"rule": "mod", "expr": "__import__('os')"})


class Band(unittest.TestCase):
    def test_each_instrument_weighs_one(self):
        # 300 notes of one cello at 0 and 30 of another at 10: weighing each instrument as
        # one, the band spans both (unweighted, its 90th percentile would sit at 0).
        pairs = [("big", 0.0)] * 300 + [("small", 10.0)] * 30
        b = band.band_scalar(pairs)
        self.assertEqual(b["n_inst"], 2)
        self.assertLess(b["p10"], 0.5)
        self.assertGreater(b["p90"], 9.5)

    def test_placement(self):
        b = {"p10": 1.0, "p50": 2.0, "p90": 3.0}
        self.assertEqual(band.place(2.5, b)["status"], "in")
        p = band.place(4.0, b)
        self.assertEqual((p["status"], p["outside"], p["widths"]), ("above", 1.0, 0.5))
        self.assertEqual(band.place(0.0, b)["status"], "below")

    def test_bands_vectors_and_compare(self):
        rng = np.random.default_rng(3)
        refs = [{"group": "g", "inst": f"i{k % 3}", "f": {"x.s": float(rng.normal(0, 1)), "x.v": list(rng.normal(0, 1, 4))}} for k in range(60)]
        near = [{"group": "g", "inst": "c", "f": {"x.s": 0.1, "x.v": [0.0, 0.1, -0.1, 0.0]}}]
        far = [{"group": "g", "inst": "c", "f": {"x.s": 5.0, "x.v": [5.0, 5.0, 0.0, 0.0]}}]
        sn, sf = band.summarise(band.bands(refs, near)), band.summarise(band.bands(refs, far))
        self.assertEqual(sn["x.s"]["in"], 1)
        self.assertEqual(sn["x.v"]["in_share"], 1.0)
        self.assertEqual(sf["x.s"]["in"], 0)
        self.assertAlmostEqual(sf["x.v"]["in_share"], 0.5)
        c = band.compare(sn, sf)
        self.assertEqual(c["x.s"]["verdict"], "closer")
        self.assertEqual(band.compare(sn, sn)["x.s"]["verdict"], "same")


class Listening(unittest.TestCase):
    def test_binomial_tail_and_critical_counts(self):
        self.assertAlmostEqual(abx_read.p_at_least(12, 16), 0.0384, places=4)
        self.assertAlmostEqual(abx_read.p_at_least(9, 10), 0.0107, places=4)
        self.assertEqual(abx_read.critical(16, 0.05), 12)
        self.assertEqual(abx_read.critical(20, 0.05), 15)
        self.assertEqual(abx_read.critical(10, 0.05), 9)

    def test_power(self):
        # P(X >= 12 | n = 16, p = 0.8) = 0.7982
        self.assertAlmostEqual(abx_read.power(16, 0.8, 0.05), 0.7982, places=3)
        n = abx_read.trials_for(0.7, 0.05, 0.8)
        self.assertGreaterEqual(abx_read.power(n, 0.7, 0.05), 0.8)
        self.assertLess(abx_read.power(n - 1, 0.7, 0.05), 0.8)

    def test_mean_ci_and_sign_test(self):
        m, h = abx_read.mean_ci([1, 2, 3, 4, 5])
        self.assertAlmostEqual(m, 3.0)
        self.assertAlmostEqual(h, 2.776 * np.std([1, 2, 3, 4, 5], ddof=1) / np.sqrt(5), places=6)
        self.assertAlmostEqual(abx_read.sign_test(8, 0), 2 / 256)

    def test_mushra_post_screening(self):
        key = abx_read.Key({"tokens": {"h": "recording", "r": "recording", "c": "cand", "a": "anchor35"}, "hidden_reference": "h"})
        good = {"mushra": [{"phrase": f"i{k}", "order": ["h", "c", "a"], "scores": {"h": 100, "c": 60, "a": 20}} for k in range(8)]}
        bad = {"mushra": [{"phrase": f"i{k}", "order": ["h", "c", "a"], "scores": {"h": 70 if k < 2 else 95, "c": 60, "a": 80}} for k in range(8)]}
        _, _, ps = abx_read.mushra(good, key)
        self.assertFalse(ps["excluded"])
        self.assertEqual(ps["anchor_flags"], [])
        _, _, ps = abx_read.mushra(bad, key)
        self.assertTrue(ps["excluded"])  # 2 of 8 = 25 % under 90
        self.assertEqual(len(ps["anchor_flags"]), 2)


if __name__ == "__main__":
    unittest.main()
