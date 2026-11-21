#!/bin/bash
# List the CI clones inside WSL: total size, target size, last commit date.
cd "$HOME" || exit 1
for d in cantus-ci* ci-*; do
  [ -d "$d" ] || continue
  t=$(du -sh "$d/target" 2>/dev/null | cut -f1)
  a=$(du -sh "$d" 2>/dev/null | cut -f1)
  l=$(git -C "$d" log -1 --format='%cr %h' 2>/dev/null)
  echo "$d  total=$a target=$t  last=$l"
done
df -h / | tail -1
