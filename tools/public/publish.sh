#!/usr/bin/env bash
# Publish the private repository's `master` to the public one, rewritten (ADR 0049, amended
# 2026-10-20): the private repository (luis314159/cantus) stays the working one and keeps
# everything; the public one (luis314159/cantus-daw) only ever receives this script's output.
#
#   bash tools/public/publish.sh <private-folder> [--push]
#
# <private-folder> holds the owner's identities.mailmap and forbid.txt (outside every
# repository). Without --push it stops after the check, leaving the rewritten clone in
# $PUBLISH_DIR (default F:/projects/going-public/cantus-daw) to look at.
#
# The rewrite is deterministic: the same commits, mailmap and rules always give the same
# hashes, so each run's `master` is a fast-forward of the last one published — this script
# refuses to push anything else. Never push the private repository's refs to the public
# remote by hand: only this script's clone may hold that remote.
#
# Needs: git-filter-repo 2.47.0 (`python -m git_filter_repo`), and gitleaks for check.sh
# ($GITLEAKS). The day-of tree changes (the screenshots and es.txt ignored, the links to the
# screenshots plain text) are already in the private `master`, so nothing is committed here.
set -euo pipefail

PRIVATE="${1:?usage: publish.sh <private-folder> [--push]}"
PUSH="${2:-}"
SRC="$(git -C "$(dirname "$0")/../.." rev-parse --show-toplevel)"
DIR="${PUBLISH_DIR:-F:/projects/going-public/cantus-daw}"
PUBLIC_URL="https://github.com/luis314159/cantus-daw.git"
TOOLS="$SRC/tools/public"

for f in "$PRIVATE/identities.mailmap" "$PRIVATE/forbid.txt"; do
  [ -f "$f" ] || { echo "missing $f" >&2; exit 1; }
done
[ -n "${GITLEAKS:-}" ] || { echo "set GITLEAKS to gitleaks.exe (check.sh needs it)" >&2; exit 1; }

# 1. A fresh clone of master alone (never the working tree: only committed objects).
rm -rf "$DIR"
mkdir -p "$(dirname "$DIR")"
git clone -q --no-local --single-branch --branch master "$SRC" "$DIR"

# 2. The RVR1960 replacement rules, built from the private es.txt, kept outside the clone.
RULES="$(dirname "$DIR")/motto.replace"
python "$TOOLS/motto_replacements.py" "$SRC/crates/cantus-ui/locales/verses" "$RULES"

# 3. The rewrite: the same paths and rules every time (scan.py --filter-repo-args prints the
#    regexes; they must match scan_rules.py).
( cd "$DIR"
  python -m git_filter_repo --quiet --invert-paths \
    --path-regex '(?i)^research/(.*/)?screenshots/' \
    --path-regex '(?i)^research/.*\.(png|jpe?g|gif|webp|bmp|tiff?|avif|heic|ico|svg|pdf|psd)$' \
    --path-regex '(?i)^crates/cantus-ui/locales/verses/es\.txt$' \
    --replace-text "$RULES" \
    --mailmap "$PRIVATE/identities.mailmap"
  [ -z "$(git remote)" ] || { echo "the clone still has a remote" >&2; exit 1; } )

# 4. The proof: nothing forbidden in any commit, no secret, the tree clean.
bash "$TOOLS/check.sh" --forbid "$PRIVATE/forbid.txt" "$DIR" master --tags

# 5. The push: only a fast-forward of what the public repository already has.
if [ "$PUSH" = "--push" ]; then
  git -C "$DIR" remote add public "$PUBLIC_URL"
  if git -C "$DIR" fetch -q public master 2>/dev/null; then
    git -C "$DIR" merge-base --is-ancestor public/master master \
      || { echo "refused: master is not a fast-forward of the public master" >&2; exit 1; }
  fi
  git -C "$DIR" push public master
fi
