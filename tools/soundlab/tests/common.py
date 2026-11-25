"""The tests' imports: the lab's package and the synthetic signals, under `python -I`."""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
LAB = HERE.parent
for p in (str(LAB), str(HERE)):
    if p not in sys.path:
        sys.path.insert(0, p)

lab = LAB
REPO = LAB.parents[1]
