#!/usr/bin/env bash
# r42 arcus-classical — Arcus's four classical instruments through the sound lab (ADR 0160) at
# their defaults: the held-out notes rendered (the cello and the bass by `arcus_match render`,
# the violin and the viola by `arcus_upper_listen <dir> study`, every fourth key), measured and
# judged on the held-out cells, then the classical pass's rows side by side with an earlier run
# (`summary.py`).
#
#   bash tools/arcus/classical/lab.sh <run> [<bin dir>] [<before run>]
#
# <bin dir> holds the two examples already built (default target/release/examples: build them
# with `tools/build/slot.sh cargo build --release -p cantus-dsp --example arcus_match --example
# arcus_upper_listen`); <before run> defaults to the analysis's (`analisis-clasico`). The cello's
# subject is the lab's (`tools/soundlab/subjects/arcus-cello`); the violin's, the viola's and the
# bass's are this pass's (`subjects/`, the analysis's scratch subjects kept here). Renders and
# reports go to $CANTUS_WORK/arcus-classical/runs/<run>/ and the lab's own run folders.
# INSTS (default all four) names the instruments to measure, e.g. INSTS="violin cello";
# ARCUS_SHAPE=name=value,… is handed to the renders (a stroke shape to try, `shape::Knobs`).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
run=$1
bin=${2:-$repo/target/release/examples}
before=${3:-analisis-clasico}
work=${CANTUS_WORK:-/f/projects/cantus-work}
out=$work/arcus-classical/runs/$run
py=${CANTUS_PY:-/f/projects/cantus-refs/venv/Scripts/python.exe}
insts=${INSTS:-violin viola cello bass}
mkdir -p "$out"
case " $insts " in *" cello "* | *" bass "*)
  "$bin/arcus_match" render "$out/lower" > "$out/render-lower.log" 2>&1 ;;
esac
case " $insts " in *" violin "* | *" viola "*)
  "$bin/arcus_upper_listen" "$out/upper" study > "$out/render-upper.log" 2>&1 ;;
esac
subj() {
  case $1 in
    cello) echo "$repo/tools/soundlab/subjects/arcus-cello/subject.json" ;;
    *) echo "$here/subjects/arcus-$1/subject.json" ;;
  esac
}
from() {
  case $1 in
    cello | bass) echo "$out/lower" ;;
    *) echo "$out/upper/study/$1" ;;
  esac
}
lab() {
  SOUNDLAB_WORKERS=${SOUNDLAB_WORKERS:-2} "$py" -I "$repo/tools/soundlab/lab.py" all "$(subj "$1")" "$run" \
    --from "$(from "$1")" --before "$before" > "$out/lab-$1.log" 2>&1
}
set -- $insts
while [ $# -gt 0 ]; do
  lab "$1" &
  if [ $# -gt 1 ]; then
    lab "$2" &
    shift
  fi
  shift
  wait
done
sl=$work/soundlab
cols=()
for i in $insts; do
  cols+=("$i<=$sl/arcus-$i/runs/$before" "$i=$sl/arcus-$i/runs/$run")
done
"$py" -I "$here/summary.py" "$out/summary.json" "${cols[@]}" | tee "$out/summary.txt"
