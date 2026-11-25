#!/usr/bin/env bash
# The bowed-strings study's measured database rebuilt from the references (round 42,
# sound-data): what tools/arcus/study's README runs by hand, in its order, into
# $CANTUS_STUDY_REFDB (default $CANTUS_REFS/derived/refdb) — never into the repository.
#
#   bash tools/sound-refs/rebuild_refdb.sh [--fresh] [--pass5]
#
# Steps: validate_synth.py (the estimator on synthetic notes) → catalog.py (every reference
# file as a job, absolute paths under $CANTUS_REFS) → extract.py (per-note records, resumable;
# STUDY_WORKERS parallel, 3) → phrases.py → aggregate.py (the four families' bands, summary) →
# plots.py → cnsm.py, friction.py, bowing.py; with --pass5 also tools/arcus/pass5's vibrato.py
# and strokes.py refs (vibrato.json, strokes.json). --fresh empties notes/ first (extract
# skips every note already recorded). Then compare.py sets every committed distribution in
# research/instruments-next/bowed-refdb/ beside the rebuilt one (exit 1 when one differs).
# About 30 minutes on 3 workers for the ~12 000 jobs, and 25 more with --pass5.
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
. "$here/tools/sound-refs/cantus_paths.sh"
fresh=0 pass5=0
for a in "$@"; do
  case $a in
    --fresh) fresh=1 ;;
    --pass5) pass5=1 ;;
    *) echo "usage: rebuild_refdb.sh [--fresh] [--pass5]" >&2; exit 2 ;;
  esac
done
db=$CANTUS_STUDY_REFDB
py=$CANTUS_PY
cd "$here"
mkdir -p "$db/notes" "$db/plots"
[ $fresh = 1 ] && rm -f "$db"/notes/*.jsonl "$db"/notes/errors.log
s=tools/arcus/study
echo "refdb: $db   references: $CANTUS_REFS   python: $py"
"$py" -I $s/validate_synth.py "$db/validation.json"
"$py" -I $s/catalog.py "$db/catalog.json"
"$py" -I $s/extract.py --workers "${STUDY_WORKERS:-3}" > "$db/extract.log" 2>&1
"$py" -I $s/phrases.py "$db/phrases.jsonl" > "$db/phrases.log" 2>&1
"$py" -I $s/aggregate.py
MPLBACKEND=Agg "$py" -I $s/plots.py
"$py" -I $s/cnsm.py > "$db/cnsm.log" 2>&1
"$py" -I $s/friction.py
"$py" -I $s/bowing.py
if [ $pass5 = 1 ]; then
  ARCUS_WORKERS=${STUDY_WORKERS:-3} "$py" -I tools/arcus/pass5/vibrato.py refs "$db/vibrato.json" > "$db/vibrato.log" 2>&1
  "$py" -I tools/arcus/pass5/strokes.py refs "$CANTUS_REFS" "$db/strokes.json" > "$db/strokes.log" 2>&1
fi
"$py" -I tools/sound-refs/compare_refdb.py "$db"
