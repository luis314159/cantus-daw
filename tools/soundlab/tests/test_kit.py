"""The blind kit's guarantees (docs/round40/abx5.md), on the worked example's subject built
into a scratch folder: one loudness per item under the ceiling, random names with no
structure (kit 4's leak caught), the veil the page reads, catch questions and the hidden
reference as one clip under two names, the key outside the kit, nothing in the page's data
naming a source."""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

from common import lab, REPO
from soundlab import kit, audio
from soundlab.loudness import lufs
from soundlab.subject import Subject

EXAMPLE = lab / "subjects" / "example-tibia"


class Blinding(unittest.TestCase):
    def test_kit_fours_names_are_caught(self):
        # names.rs's test vector: kit 4's names (an FNV of phrase|piano) share 15 of 16 digits.
        k4 = ("11e73706bbfbd9ca 2c0dba9592e70815 2c0dbb9592e702c2 34dc5adcf31ef7f6 34dc5bdcf31efc25 "
              "3910eabe1ac487e0 3910ebbe1ac48e2f").split()
        self.assertIsNotNone(kit.names_look_random(k4))
        self.assertIsNotNone(kit.names_look_random(k4[1:3]))

    def test_drawn_names_look_random(self):
        for _ in range(50):
            self.assertIsNone(kit.names_look_random(kit.draw_names(40)))
            self.assertTrue(all(re.fullmatch("[0-9a-f]{16}", n) for n in kit.draw_names(3)))

    def test_veil_matches_names_rs_and_inverts(self):
        # base64 per RFC 4648 of the plain bytes is not the veil; the page's unveil inverts it.
        t = json.dumps({"a1b2c3d4": "recording", "e5f6a7b8": "candidate"})
        v = kit.veil(t, 12345)
        self.assertNotIn("recording", v)
        self.assertEqual(kit.unveil(v, 12345), t)
        # The JS page's unveil (abx.html), run by node when present, reads the same table.
        node = shutil.which("node")
        if node:
            js = ("const b=Buffer.from(process.argv[1],'base64');let x=12345>>>0;for(let i=0;i<b.length;i++)"
                  "{x=(x^(x<<13))>>>0;x=(x^(x>>>17))>>>0;x=(x^(x<<5))>>>0;b[i]^=x>>>24;}process.stdout.write(b.toString('utf8'))")
            self.assertEqual(subprocess.run([node, "-e", js, v], capture_output=True, text=True, check=True).stdout, t)


class Matching(unittest.TestCase):
    def test_a_group_is_cut_faded_and_matched(self):
        # kit.rs's test: two noises 12 dB apart, of two lengths -> one length, one loudness.
        rng = np.random.default_rng(1)
        a = 0.4 * (rng.random((96000, 2)) - 0.5)
        b = 0.1 * (rng.random((90000, 2)) - 0.5)
        out, target, before, gains = kit.match_group([a, b], 0.03, -20.0, -1.0)
        self.assertEqual(len(out[0]), 90000)
        self.assertEqual(out[0][-1, 0], 0.0)
        self.assertAlmostEqual(before[0][0] - before[1][0], 12.04, delta=0.2)
        for c in out:
            self.assertLess(abs(lufs(c, 48000) - target), 0.05)
            self.assertLessEqual(20 * np.log10(np.abs(c).max()), -1.0 + 1e-9)

    def test_a_loud_peak_lowers_the_group(self):
        t = np.arange(48000) / 48000
        quiet = np.repeat((0.05 * np.sin(2 * np.pi * 440 * t))[:, None], 2, 1)
        spiky = quiet.copy()
        spiky[1000] = 0.99
        out, target, _, _ = kit.match_group([quiet, spiky], 0.01, -14.0, -1.0)
        self.assertLess(target, -14.0)
        self.assertLessEqual(max(20 * np.log10(np.abs(c).max()) for c in out), -1.0 + 1e-9)


class TheExampleKit(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = Path(tempfile.mkdtemp(prefix="soundlab-kit-"))
        cls.env = dict(os.environ)
        os.environ["SOUNDLAB_REFS"] = str(cls.tmp / "refs")
        os.environ["SOUNDLAB_WORK"] = str(cls.tmp / "work")
        subprocess.run([sys.executable, "-I", str(EXAMPLE / "synth_bar.py"), "refs", str(cls.tmp / "refs" / "soundlab-example")],
                       check=True, capture_output=True)
        cls.s = Subject(EXAMPLE / "subject.json")
        cls.out = cls.tmp / "kit"
        kit.build(cls.s, cls.out, flac=True, twin=cls.tmp / "kit-wav", run="t", log=lambda *_: None)
        cls.key = json.loads((cls.tmp / "kit-key" / "key.json").read_text(encoding="utf-8"))
        html = (cls.out / "index.html").read_text(encoding="utf-8")
        cls.const = {k: re.search(rf"const {k} = (.*);\r?\n", html).group(1) for k in ("KIT", "PHRASES", "FILES", "TESTS", "SEED", "REVEAL", "SUBJECT")}
        cls.html = html

    @classmethod
    def tearDownClass(cls):
        os.environ.clear()
        os.environ.update(cls.env)
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def test_only_what_a_listener_receives(self):
        self.assertEqual(sorted(p.name for p in self.out.iterdir()), ["LEEME.md", "README.md", "audio", "index.html"])
        self.assertFalse(any(p.name == "key.json" for p in self.out.rglob("*")))
        self.assertTrue((self.tmp / "kit-key" / "levels.csv").exists())

    def test_every_placeholder_filled_and_every_file_named(self):
        for p in kit.PLACEHOLDERS:
            self.assertNotIn(p, self.html)
        files = json.loads(self.const["FILES"])
        on_disk = sorted(p.name for p in (self.out / "audio").iterdir())
        self.assertEqual(sorted(files.values()), on_disk)
        self.assertIsNone(kit.names_look_random([n[:16] for n in on_disk]))
        twin = sorted(p.name for p in (self.tmp / "kit-wav" / "audio").iterdir())
        self.assertEqual([n[:16] for n in twin], [n[:16] for n in on_disk])

    def test_one_loudness_per_item_under_the_ceiling(self):
        files = json.loads(self.const["FILES"])
        for it in json.loads(self.const["PHRASES"]):
            levels = []
            for k, fn in files.items():
                if k.startswith(it + "|"):
                    x, sr = audio.read(self.out / "audio" / fn)
                    levels.append(lufs(x, sr))
                    self.assertLessEqual(20 * np.log10(np.abs(x).max()), -1.0 + 0.01)
            self.assertLess(max(levels) - min(levels), 0.1, (it, levels))
            self.assertAlmostEqual(np.mean(levels), self.key["items"][it]["target_lufs"], delta=0.1)

    def test_catches_and_the_hidden_reference_are_one_clip_twice(self):
        tests = json.loads(self.const["TESTS"])
        files = json.loads(self.const["FILES"])
        table = json.loads(kit.unveil(json.loads(self.const["REVEAL"]), int(self.const["SEED"])))
        self.assertEqual(table, self.key["tokens"])
        self.assertEqual(len(tests["catch"]), 2)
        for c in tests["catch"]:
            a, b = files[f"{c['phrase']}|{c['one']}"], files[f"{c['phrase']}|{c['two']}"]
            self.assertNotEqual(a, b)
            self.assertEqual((self.out / "audio" / a).read_bytes(), (self.out / "audio" / b).read_bytes())
        m = tests["mushra"]
        hidden = [t for t in m["stims"] if t != m["ref"] and table[t] == table[m["ref"]]]
        self.assertEqual(len(hidden), 1)
        for it in m["items"]:
            a, b = files[f"{it}|{m['ref']}"], files[f"{it}|{hidden[0]}"]
            self.assertEqual((self.out / "audio" / a).read_bytes(), (self.out / "audio" / b).read_bytes())
        self.assertIn("anchor35", table.values())

    def test_the_pages_data_names_no_source(self):
        plain = "\n".join(self.const[k] for k in ("KIT", "PHRASES", "FILES", "TESTS", "SEED", "REVEAL")).lower()
        for w in json.loads(self.const["SUBJECT"])["hide"]["data"]:
            self.assertNotIn(w, plain)
        for f in ("LEEME.md", "README.md"):
            t = (self.out / f).read_text(encoding="utf-8")
            for w in json.loads(self.const["SUBJECT"])["hide"]["readme"]:
                self.assertNotIn(w, t)


@unittest.skipIf(os.environ.get("SOUNDLAB_LIGHT") == "1", "light run")
class EverySubjectIsAPageSubject(unittest.TestCase):
    def test_the_labs_subjects(self):
        for p in (lab / "subjects").glob("*/subject.json"):
            s = Subject(p)
            if "listening" in s.d:
                page = kit.page_subject(s.d["listening"])
                for lang in ("es", "en"):
                    for v in {v for pair in page["pairs"] for v in pair}:
                        self.assertIn(f"voice_{v}", page["text"][lang], (p, v))
                    self.assertIn(f"q_{page['play']}", page["text"][lang], p)
                    for it in s.d["listening"]["items"]:
                        self.assertIn(it, page["items"][lang], (p, it))
        self.assertTrue((REPO / "crates" / "cantus-ui" / "examples" / "abx_kit" / "abx.html").exists())


if __name__ == "__main__":
    unittest.main()
