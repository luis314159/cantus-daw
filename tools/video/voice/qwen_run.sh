#!/usr/bin/env bash
# The Qwen3-TTS takes, one model in memory at a time (the machine is short of RAM):
#   1. two designed Latin American Spanish voices, four reference clips each, gated;
#   2. the English takes in two built-in voices (Aiden, Ryan);
#   3. the Spanish takes in each designed voice, from its best gated reference clip;
#   4. the gate over both; 5. the comparison page beside the Chatterbox takes.
#
#   bash tools/video/voice/qwen_run.sh /f/projects/video-work [from-step]
set -eu
WORK=${1:?work folder}
FROM=${2:-1}
PY=$WORK/env/qwen/Scripts/python.exe
M=$WORK/models
V=$WORK/voice
T=tools/video/voice
export OMP_NUM_THREADS=2 MKL_NUM_THREADS=2 HF_HOME=$WORK/cache/hf
# the gate's ear: large-v3-turbo (1.6 GB) by default, large-v3 (3.1 GB) with WHISPER=large-v3
WHISPER=${WHISPER:-large-v3-turbo}

if [ "$FROM" -le 1 ]; then
  for v in es-latam es-latam-f; do
    $PY -I $T/takes_qwen.py design-ref --model $M/qwen3-tts-1.7b-voicedesign --voice $v --lang es --out $V/qwen/ref --takes 4
  done
  $PY -I $T/gate.py --takes $V/qwen/ref --whisper $M/whisper --model $WHISPER
fi
if [ "$FROM" -le 2 ]; then
  for sp in Aiden Ryan; do
    $PY -I $T/takes_qwen.py custom --model $M/qwen3-tts-1.7b-customvoice --speaker $sp --lang en \
      --out $V/qwen/en --takes 5 --tag "$(echo $sp | tr A-Z a-z)-"
  done
fi
if [ "$FROM" -le 3 ]; then
  for v in es-latam es-latam-f; do
    # the voice's passing reference clip nearest a calm pace
    ref=$($PY -I -c "
import json, sys
rows = [r for r in json.load(open(sys.argv[1], encoding='utf-8')) if r['take'].startswith(sys.argv[2] + '-') and r['take'][len(sys.argv[2]) + 1:-4].isdigit()]
ok = sorted((r for r in rows if r['pass']), key=lambda r: abs(r['rate'] - 5.0))
print(ok[0]['take'] if ok else '')" "$V/qwen/ref/gate.json" "$v")
    [ -n "$ref" ] || { echo "no reference of $v passed the gate"; continue; }
    echo "voice $v from $ref"
    tag=$([ "$v" = es-latam ] && echo m- || echo f-)
    $PY -I $T/takes_qwen.py from-ref --model $M/qwen3-tts-1.7b-base --ref "$V/qwen/ref/$ref" --lang es \
      --out $V/qwen/es --takes 5 --tag "$tag"
  done
fi
if [ "$FROM" -le 4 ]; then
  for l in en es; do $PY -I $T/gate.py --takes $V/qwen/$l --whisper $M/whisper --model $WHISPER; done
fi
$PY -I $T/compare.py --pair en $V/pool/en $V/qwen/en --pair es $V/pool/es $V/qwen/es --out $V/ab-qwen.html
