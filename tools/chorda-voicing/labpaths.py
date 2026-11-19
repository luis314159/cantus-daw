"""Where the voicing lab's files live (round 40) — every script here imports its paths from
this one place, so nothing names a machine's folders. Override with the environment:

    CHORDA_LAB     the lab's working folder: renders and results (default
                   <repo>/target/chorda-lab — under target/, so git never sees it)
    CHORDA_SHARE   where previews and listening kits are written (default <lab>/share)
    CHORDA_TMP     scratch (default <lab>/tmp)
    CANTUS_PY      the Python that runs the analyses (default: this one)
    CHORDA_EXE     the voicing lab, `chorda_compare` built in release (default
                   <repo>/target/release/examples/chorda_compare[.exe])

The lab's layout (what `env.sh` and the scripts expect inside CHORDA_LAB): `eng40/wav/<src>/`
the engine's renders of the listening phrases (`salamander`, `chorda2`), `notes5/wav/<src>/`
the single notes (notes_pp/mf/ff), `keys/wav/<src>/keys_mf.wav` every key at mf, `lab4/wav/<tag>/`
a candidate's phrases, `ana/out_<tag>/` and `ana2/out_<tag>/` the two analyses' results.
"""
import os
import sys

__all__ = ['TOOLS', 'REPO', 'LAB', 'SHARE', 'TMP', 'PY', 'EXE', 'C16', 'tool_path']

TOOLS = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(TOOLS))
LAB = os.environ.get('CHORDA_LAB') or os.path.join(REPO, 'target', 'chorda-lab')
SHARE = os.environ.get('CHORDA_SHARE') or os.path.join(LAB, 'share')
TMP = os.environ.get('CHORDA_TMP') or os.path.join(LAB, 'tmp')
PY = os.environ.get('CANTUS_PY') or sys.executable
EXE = os.environ.get('CHORDA_EXE') or os.path.join(
    REPO, 'target', 'release', 'examples', 'chorda_compare' + ('.exe' if os.name == 'nt' else ''))
# Candidate 16 (the shipped voicing since DSP_VERSION 7): the worked example and every
# probe's default base.
C16 = os.path.join(TOOLS, 'candidates', 'c16')


def tool_path(*parts):
    """A path inside tools/chorda-voicing."""
    return os.path.join(TOOLS, *parts)
