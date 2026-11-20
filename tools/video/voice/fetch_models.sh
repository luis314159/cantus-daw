#!/usr/bin/env bash
# Download the voice models at their pinned revisions into $WORK/models/<name>/, one folder
# each, then check every large file against models.txt's SHA-256. Behind a proxy that
# quarantines a download (an HTML page comes back for ~10 minutes), the loop waits and
# retries; nothing here bypasses a proxy or TLS.
#
#   bash tools/video/voice/fetch_models.sh /f/projects/video-work [chatterbox|whisper|qwen]...
set -u
WORK=${1:?work folder}; shift
M=$WORK/models
want=${*:-chatterbox whisper qwen}
here=$(cd "$(dirname "$0")" && pwd)

fetch() { # url file
  mkdir -p "$(dirname "$2")"
  for try in 1 2 3 4 5 6; do
    curl -sSL --retry 3 -o "$2" "$1" && ! head -c 15 "$2" | grep -qi "<!doctype\|<html" && return 0
    echo "  $2: not the file yet (try $try), waiting a minute"; sleep 60
  done
  return 1
}
hf() { # repo revision folder files...
  local repo=$1 rev=$2 dir=$3; shift 3
  for f in "$@"; do fetch "https://huggingface.co/$repo/resolve/$rev/$f" "$M/$dir/$f" && echo "$dir/$f"; done
}

QWEN_FILES="config.json generation_config.json merges.txt preprocessor_config.json tokenizer_config.json vocab.json README.md speech_tokenizer/config.json speech_tokenizer/configuration.json speech_tokenizer/preprocessor_config.json speech_tokenizer/model.safetensors model.safetensors"
for w in $want; do
  case $w in
    chatterbox) hf ResembleAI/chatterbox 5bb1f6ee58e50c3b8d408bc82a6d3740c2db6e18 chatterbox \
        ve.pt t3_mtl23ls_v2.safetensors s3gen.pt grapheme_mtl_merged_expanded_v1.json conds.pt Cangjie5_TC.json ;;
    whisper) fetch https://openaipublic.azureedge.net/main/whisper/models/e5b1a55b89c1367dacf97e3e19bfd829a01529dbfdeefa8caeb59b3f1b81dadb/large-v3.pt \
        "$M/whisper/large-v3.pt" && echo whisper/large-v3.pt ;;
    qwen)
      hf Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice 0c0e3051f131929182e2c023b9537f8b1c68adfe qwen3-tts-1.7b-customvoice $QWEN_FILES
      hf Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign 5ecdb67327fd37bb2e042aab12ff7391903235d3 qwen3-tts-1.7b-voicedesign $QWEN_FILES
      hf Qwen/Qwen3-TTS-12Hz-1.7B-Base fd4b254389122332181a7c3db7f27e918eec64e3 qwen3-tts-1.7b-base $QWEN_FILES ;;
  esac
done

# Every file models.txt gives a hash for, checked.
grep -E '^[a-z0-9].* [0-9a-f]{64}$' "$here/models.txt" | while read -r file size sha; do
  [ -f "$M/$file" ] || continue
  got=$(sha256sum "$M/$file" | cut -d' ' -f1)
  [ "$got" = "$sha" ] && echo "ok   $file" || echo "BAD  $file ($got)"
done
