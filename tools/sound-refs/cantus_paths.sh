# Where the sound tools find their data (round 42, sound-data) — the shell side of
# tools/sound-refs/cantus_paths.py. Source it from a script under tools/:
#
#   . "$(cd "$(dirname "${BASH_SOURCE[0]}")/../sound-refs" && pwd)/cantus_paths.sh"
#
# It sets, unless already set (Git Bash paths on Windows, plain paths elsewhere):
#   CANTUS_REFS         the reference recordings (tools/sound-refs/catalog.toml), default
#                       F:\projects\cantus-refs (/f/projects/cantus-refs) on Windows,
#                       ~/cantus-refs elsewhere
#   CANTUS_WORK         scratch and renders, default `cantus-work` beside CANTUS_REFS
#   CANTUS_STUDY_REFDB  the bowed-strings study's measured database, default
#                       $CANTUS_REFS/derived/refdb
#   CANTUS_PY           the sound tools' Python (numpy, scipy, soundfile), default the venv
#                       $CANTUS_REFS/venv (tools/sound-refs/setup.md), else `python`
# and exports them, so the Python tools a script runs see the same folders.
if [ -z "${CANTUS_REFS:-}" ]; then
  if [ "${OS:-}" = "Windows_NT" ]; then
    CANTUS_REFS=/f/projects/cantus-refs
  else
    CANTUS_REFS="$HOME/cantus-refs"
  fi
fi
CANTUS_WORK=${CANTUS_WORK:-$(dirname "$CANTUS_REFS")/cantus-work}
CANTUS_STUDY_REFDB=${CANTUS_STUDY_REFDB:-$CANTUS_REFS/derived/refdb}
if [ -z "${CANTUS_PY:-}" ]; then
  if [ -x "$CANTUS_REFS/venv/Scripts/python.exe" ]; then
    CANTUS_PY="$CANTUS_REFS/venv/Scripts/python.exe"
  elif [ -x "$CANTUS_REFS/venv/bin/python" ]; then
    CANTUS_PY="$CANTUS_REFS/venv/bin/python"
  else
    CANTUS_PY=python
  fi
fi
export CANTUS_REFS CANTUS_WORK CANTUS_STUDY_REFDB CANTUS_PY
