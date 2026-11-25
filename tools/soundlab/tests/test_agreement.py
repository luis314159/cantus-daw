"""The lab against the estimator it generalises, on real recordings: the Arcus study's
per-note records (`<refdb>/notes/ircam-cello.jsonl`: SOUNDLAB_STUDY the study's folder, or
CANTUS_STUDY_REFDB the refdb itself; its files under SOUNDLAB_REFS or CANTUS_REFS) read the same TinySOL cello notes the lab reads. At the
file's own rate the port must give the study's numbers; through the lab's pipeline (48 kHz,
the note's pitch known) within the study's measured accuracy."""
import json
import os
import unittest
from pathlib import Path

import numpy as np

from common import lab, local, study_refdb  # noqa: F401
from soundlab import audio
from soundlab.features import Note, analyse


@unittest.skipUnless(study_refdb() and os.environ.get("SOUNDLAB_REFS")
                     and os.environ.get("SOUNDLAB_LIGHT") != "1", "SOUNDLAB_STUDY / SOUNDLAB_REFS not set")
class AgainstTheStudy(unittest.TestCase):
    def test_tinysol_cello_notes(self):
        notes = study_refdb() / "notes" / "ircam-cello.jsonl"
        rows = [json.loads(line) for line in notes.read_text(encoding="utf-8").splitlines() if line.strip()]
        rows = [r for r in rows if r.get("dyn") == "mf"][::8][:12]
        self.assertGreaterEqual(len(rows), 8)
        worst = {"f0": 0.0, "h": 0.0, "vib": 0.0, "noise": 0.0, "f0_48": 0.0, "h_48": 0.0}
        for r in rows:
            p = local(r["path"])
            x, sr = audio.read(p, channels=1)
            a = analyse(Note(x, sr, "sustained", (60, 1100), hint=False), ["pitch", "harmonics", "vibrato", "noise"])
            worst["f0"] = max(worst["f0"], abs(1200 * np.log2(a["pitch"]["f0_hz"] / r["f0_hz"])))
            h = np.array(a["harmonics"]["h_db_re_max"][:10], float)
            worst["h"] = max(worst["h"], float(np.nanmax(np.abs(h - np.array(r["h_db_re_max"][:10])))))
            if r.get("vibrato") and "vibrato" in a and "rate_hz" in a["vibrato"]:
                worst["vib"] = max(worst["vib"], abs(a["vibrato"]["rate_hz"] - r["vibrato"]["rate_hz"]))
            if r["noise"].get("res_valid_db_re_harm") is not None:
                worst["noise"] = max(worst["noise"], abs(a["noise"]["res_valid_db_re_harm"] - r["noise"]["res_valid_db_re_harm"]))
            # the lab's own pipeline: 48 kHz and the note known
            y, _ = audio.read(p, sr=48000, channels=1)
            m = 12 * (int(r["nominal"][-1]) + 1) + "C C# D D# E F F# G G# A A# B".split().index(r["nominal"][:-1])
            b = analyse(Note(y, 48000, "sustained", (60, 1100), nominal=m), ["pitch", "harmonics"])
            worst["f0_48"] = max(worst["f0_48"], abs(1200 * np.log2(b["pitch"]["f0_hz"] / r["f0_hz"])))
            h = np.array(b["harmonics"]["h_db_re_max"][:10], float)
            worst["h_48"] = max(worst["h_48"], float(np.nanmax(np.abs(h - np.array(r["h_db_re_max"][:10])))))
        print("\n  against the study (worst of %d notes): " % len(rows) + ", ".join(f"{k} {v:.3g}" for k, v in worst.items()))
        self.assertLess(worst["f0"], 0.01)
        self.assertLess(worst["h"], 0.01)
        self.assertLess(worst["vib"], 0.01)
        self.assertLess(worst["noise"], 0.01)
        self.assertLess(worst["f0_48"], 1.0)
        self.assertLess(worst["h_48"], 1.0)


if __name__ == "__main__":
    unittest.main()
