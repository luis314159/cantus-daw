#!/usr/bin/env bash
# release.yml's first job (docs/RELEASING.md): a tag is refused unless
# docs/releases/<tag>.preflight.md ticks every lesson docs/RELEASING.md lists, and names a
# green hand-run of release.yml on a commit the tag's differs from only in docs/releases/.
#
#   tools/release/preflight.sh v0.1.0-beta.16            # in the workflow (needs GH_TOKEN)
#   tools/release/preflight.sh v0.1.0-beta.16 --offline  # the same, without the API
set -euo pipefail

tag="${1:?usage: preflight.sh v<version> [--offline]}"
mode="${2:-}"
cd "$(dirname "$0")/../.."

fail() {
  echo "preflight: $*" >&2
  echo "preflight: see docs/RELEASING.md" >&2
  exit 1
}

list="docs/releases/$tag.preflight.md"
[ -f "$list" ] || fail "$list is missing: copy docs/releases/PREFLIGHT.md (step 3)"
text="$(tr -d '\r' < "$list")"

# Every lesson ticked, and nothing left empty.
lessons="$(tr -d '\r' < docs/RELEASING.md | sed -nE 's/^### (L[0-9]+) .*/\1/p')"
[ -n "$lessons" ] || fail "docs/RELEASING.md lists no lesson"
missing=""
for l in $lessons; do
  if ! grep -qE "^- \[[xX]\] $l " <<< "$text"; then
    missing="$missing $l"
  fi
done
[ -z "$missing" ] || fail "not ticked in $list:$missing (check each for this release)"
if grep -qE '^- \[ \]' <<< "$text"; then
  fail "$list still has an empty box"
fi

# The rehearsal: its commit, and nothing but docs/releases/ changed since.
run="$(sed -nE 's/^Rehearsal run: *([0-9]+) *$/\1/p' <<< "$text")"
sha="$(sed -nE 's/^Rehearsal commit: *([0-9a-f]{40}) *$/\1/p' <<< "$text")"
[ -n "$run" ] || fail "$list names no rehearsal run (its number)"
[ -n "$sha" ] || fail "$list names no rehearsal commit (its full 40-character id)"
git cat-file -e "$sha^{commit}" 2> /dev/null || fail "commit $sha is not in this repository"
git merge-base --is-ancestor "$sha" HEAD || fail "the rehearsal's $sha is not under the tag"
changed="$(git diff --name-only "$sha" HEAD | grep -v '^docs/releases/' || true)"
if [ -n "$changed" ]; then
  fail "changed since the rehearsal, outside docs/releases/ (rehearse again): $(echo $changed)"
fi

if [ "$mode" = "--offline" ]; then
  echo "preflight: $tag's checklist is complete; rehearsal $run on $sha not asked of GitHub (--offline)"
  exit 0
fi

# The rehearsal ran release.yml by hand, on that commit, and passed.
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is not set}"
info="$(gh api "repos/$GITHUB_REPOSITORY/actions/runs/$run" \
  --jq '[.path, .event, .status, .conclusion // "", .head_sha] | @tsv')" \
  || fail "run $run could not be read from GitHub"
IFS=$'\t' read -r path event status conclusion head <<< "$info"
[ "$path" = ".github/workflows/release.yml" ] || fail "run $run is $path, not release.yml"
[ "$event" = "workflow_dispatch" ] || fail "run $run was started by $event, not by hand"
[ "$status" = "completed" ] && [ "$conclusion" = "success" ] \
  || fail "run $run is $status/$conclusion, not a green rehearsal"
[ "$head" = "$sha" ] || fail "run $run ran on $head, not on $sha"
echo "preflight: $tag — every lesson ticked; rehearsal $run green on $sha"
