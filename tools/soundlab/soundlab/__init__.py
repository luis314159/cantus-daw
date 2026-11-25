"""The sound lab (round 42, ADR 0160): one instrument-agnostic place to measure an
instrument against recordings and to build its blind listening kit from a subject file.

Modules:
- `audio`     reading, mono/stereo, resampling, writing
- `core`      the signal core (YIN, the phase-tracked harmonic projection, band powers),
              ported from the Arcus reference study (`tools/arcus/study/bowlib_core.py`)
- `loudness`  ITU-R BS.1770-4 / EBU R128: integrated, momentary, short-term, LRA, true peak
- `features`  the documented estimators, one registry (`FEATURES`), per note
- `band`      the held-out band method: the seeded split, the real-instrument spread, the
              placement, before -> after
- `subject`   the subject file: references' catalogue, features, the candidate's render
- `report`    the report as JSON and as a readable HTML page with plots
- `kit`       the blind kit (ABX, preferences, MUSHRA) on the shared page
              `crates/cantus-ui/examples/abx_kit/abx.html`

Run every script with `python -I` (numpy, scipy, soundfile: requirements.txt).
"""

VERSION = "3"
