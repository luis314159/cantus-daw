#!/bin/bash
# Remove the stale CI clones inside WSL (clones of the main repository, checked out with
# --force by tools/wsl/check.sh: no work of anyone's lives in them). ~/cantus-ci, the active
# one, stays. Prints what it removes and the space after, then trims the free blocks.
cd "$HOME" || exit 1
for d in cantus-ci-ara cantus-ci-fx cantus-ci-hotfix cantus-ci-macos cantus-ci-r33i \
         cantus-ci-r41c cantus-ci-release cantus-ci-ship ci-r38-integ; do
  [ -d "$d" ] || continue
  if [ -n "$(git -C "$d" status --porcelain 2>/dev/null | grep -v '^??' | head -1)" ]; then
    echo "KEPT $d: it has uncommitted changes"
    continue
  fi
  rm -rf "$d" && echo "removed $d"
done
df -h / | tail -1
