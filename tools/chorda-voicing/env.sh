# The voicing lab's environment (round 40) — sourced by every shell script here; see
# labpaths.py for what each variable means. Bash (Git Bash on Windows, or Linux).
#   V   the lab (CHORDA_LAB), POSIX form, for the shell    VW  the same for native programs
#   T   tools/chorda-voicing                               TW  the same for native programs
#   PY  CANTUS_PY      CHORDA_EXE  the voicing lab        C16_SPEC/PARTIALS/LEVELS  candidate 16
_here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
T=$_here
REPO=$(cd "$T/../.." && pwd)
_native() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s\n' "$1"; fi; }
_posix() { if command -v cygpath >/dev/null 2>&1; then cygpath -u "$1"; else printf '%s\n' "$1"; fi; }
V=$(_posix "${CHORDA_LAB:-$REPO/target/chorda-lab}")
mkdir -p "$V"
VW=$(_native "$V")
TW=$(_native "$T")
: "${CANTUS_PY:=python}"
PY=$CANTUS_PY
if [ -z "${CHORDA_EXE:-}" ]; then
  CHORDA_EXE=$REPO/target/release/examples/chorda_compare
  [ -f "$CHORDA_EXE.exe" ] && CHORDA_EXE=$CHORDA_EXE.exe
fi
# A bare name is a copy in <lab>/bin (the round-40 habit: one build per knob set, kept by tag).
case "$CHORDA_EXE" in */*|*\\*) ;; *) CHORDA_EXE=$V/bin/$CHORDA_EXE ;; esac
export CHORDA_EXE
C16_SPEC=$TW/candidates/c16/spec.txt
C16_PARTIALS=$TW/candidates/c16/partials.csv
C16_LEVELS=$TW/candidates/c16/levels.csv
# The lab's renders are large; keep their scratch beside them, not in the system's temp.
_tmp=${CHORDA_TMP:-$V/tmp}
mkdir -p "$_tmp"
export TMP=$(_native "$_tmp") TEMP=$(_native "$_tmp")
export CHORDA_LAB=$VW
