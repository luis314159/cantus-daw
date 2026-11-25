"""Every test of the sound lab, one command:

    python -I tools/soundlab/tests/run.py [--light] [-v] [pattern]

`--light` (SOUNDLAB_LIGHT=1) skips what needs downloaded references or a generated kit.
Optional data, each test skipped when absent: SOUNDLAB_REFS (the downloads folder: TinySOL's
published pitches, the study's estimator against the lab's on reference files), SOUNDLAB_EBU
(the EBU loudness test set), SOUNDLAB_KIT_KEY (a generated kit's key folder: the kits' Rust
meter against this BS.1770). Exit 1 on any failure.
"""
import os
import sys
import unittest
from pathlib import Path

here = Path(__file__).resolve().parent
sys.path.insert(0, str(here))
import common  # noqa: E402,F401

args = [a for a in sys.argv[1:]]
if "--light" in args:
    os.environ["SOUNDLAB_LIGHT"] = "1"
    args.remove("--light")
verbosity = 2 if "-v" in args else 1
args = [a for a in args if a != "-v"]
pattern = f"test_{args[0]}*.py" if args else "test_*.py"
suite = unittest.defaultTestLoader.discover(str(here), pattern=pattern, top_level_dir=str(here))
result = unittest.TextTestRunner(verbosity=verbosity).run(suite)
sys.exit(0 if result.wasSuccessful() else 1)
