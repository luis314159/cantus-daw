#!/bin/bash
# Is this repository ready to be made public? (ADR 0049, docs/round26/public.md)
#
#   tools/public/check.sh [--forbid FILE | --no-forbid] [--no-gitleaks] [REPO_DIR] [REF ...]
#
# Read-only: it never writes to the repository. With no REF it checks every ref (the whole
# history); on the day, pass what will be published: `master --tags`.
#
#   1. history  — tools/public/scan.py over every commit of the refs: no path that must leave
#                 (research/**/screenshots/**, any image under research/, the RVR1960 file;
#                 case-insensitive), no RVR1960 text, no secret, and no entry of the owner's
#                 FORBID LIST (a file kept outside the repository: --forbid FILE, else
#                 $CANTUS_FORBID, else ~/.cantus-public/forbid.txt) in paths, blobs,
#                 messages or identities. Without a forbid list the check refuses to run
#                 unless --no-forbid is given.
#   2. gitleaks — the maintained scanner over the same commits, merges included. MANDATORY:
#                 on PATH or GITLEAKS=/path/to/gitleaks; only --no-gitleaks skips it.
#   3. tree     — in the checked-out tree: the screenshots and es.txt ignored and untracked,
#                 nothing that links to the screenshots (inline or reference-style).
#
# Exit status: 0 ready, 1 something to fix, 2 the check itself could not run.
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
forbid_args=()
gitleaks_on=1
while [ $# -gt 0 ]; do
  case "$1" in
    --forbid) [ $# -ge 2 ] || { echo "--forbid needs a file" >&2; exit 2; }
              forbid_args=(--forbid "$2"); shift 2 ;;
    --no-forbid) forbid_args=(--no-forbid); shift ;;
    --no-gitleaks) gitleaks_on=0; shift ;;
    -h|--help) sed -n '2,21p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) break ;;
  esac
done
repo="${1:-.}"
[ $# -gt 0 ] && shift
refs=("$@")
for r in "${refs[@]}"; do
  case "$r" in
    --all|--tags|--branches) ;;
    -*) echo "refusing ref option $r (only --all, --tags, --branches, or ref names)" >&2; exit 2 ;;
  esac
done
fail=0

if ! git -C "$repo" rev-parse --git-dir >/dev/null 2>&1; then
  echo "not a git repository: $repo" >&2
  exit 2
fi
repo="$(git -C "$repo" rev-parse --show-toplevel)"

py=""
for c in python3 python /c/Python313/python; do
  if command -v "$c" >/dev/null 2>&1 && "$c" -c 'import sys; sys.exit(sys.version_info < (3, 8))' 2>/dev/null; then
    py="$c"
    break
  fi
done
[ -n "$py" ] || { echo "no Python 3.8+ found (python3/python)" >&2; exit 2; }

gl=""
if [ $gitleaks_on -eq 1 ]; then
  gl="${GITLEAKS:-$(command -v gitleaks 2>/dev/null || true)}"
  if [ -z "$gl" ] || [ ! -x "$gl" ]; then
    echo "gitleaks not found: install it (GITLEAKS=/path/to/gitleaks) or pass --no-gitleaks" >&2
    exit 2
  fi
fi

echo "== 1. history (scan.py) =="
if [ ${#refs[@]} -gt 0 ]; then
  "$py" -I "$here/scan.py" --repo "$repo" "${forbid_args[@]}" --refs "${refs[@]}"
else
  "$py" -I "$here/scan.py" --repo "$repo" "${forbid_args[@]}"
fi
rc=$?
[ $rc -eq 2 ] && exit 2
[ $rc -ne 0 ] && fail=1

echo
echo "== 2. gitleaks =="
if [ $gitleaks_on -eq 1 ]; then
  if [ ${#refs[@]} -gt 0 ]; then opts="-m ${refs[*]}"; else opts="--all -m"; fi
  if "$gl" git --log-opts="$opts" --redact=50 --no-banner "$repo"; then
    echo "[ok] gitleaks: no leaks"
  else
    echo "[FAIL] gitleaks found something (run it with --report-path to see the findings)"
    fail=1
  fi
else
  echo "[SKIPPED] --no-gitleaks: this run cannot say READY for publishing"
fi

echo
echo "== 3. tree ($(git -C "$repo" rev-parse --abbrev-ref HEAD 2>/dev/null)) =="
for probe in "research/zz/screenshots/x.png" "research/zz/deeper/screenshots/y.jpg" \
             "crates/cantus-ui/locales/verses/es.txt"; do
  if git -C "$repo" check-ignore -q --no-index "$probe"; then
    echo "[ok] .gitignore keeps out $probe"
  else
    echo "[FAIL] .gitignore does not ignore $probe (add: research/**/screenshots/ and"
    echo "       crates/cantus-ui/locales/verses/es.txt)"
    fail=1
  fi
done
tracked="$(git -C "$repo" ls-files | grep -i -E '^research/(.*/)?screenshots/|^research/.*\.(png|jpe?g|gif|webp|bmp|tiff?|avif|heic|ico|svg|pdf|psd)$|^crates/cantus-ui/locales/verses/es\.txt$' | head -5)"
if [ -n "$tracked" ]; then
  echo "[FAIL] still tracked:"
  echo "$tracked" | sed 's/^/  /'
  fail=1
else
  echo "[ok] no screenshot, no image under research/ and no RVR1960 file tracked"
fi
links="$(git -C "$repo" grep -l -i -E '\]\([^)]*screenshots/|src="[^"]*screenshots/|^[[:space:]]*\[[^]]+\]:[[:space:]]*<?[^[:space:]]*screenshots/' -- '*.md' '*.html' '*.rs' 2>/dev/null)"
if [ -n "$links" ]; then
  echo "[FAIL] files that still link to the screenshots:"
  echo "$links" | sed 's/^/  /'
  fail=1
else
  echo "[ok] nothing links to the screenshots"
fi

echo
if [ $fail -ne 0 ]; then
  echo "NOT READY: see the [FAIL] lines above."
elif [ $gitleaks_on -eq 0 ] || [ "${forbid_args[0]:-}" = "--no-forbid" ]; then
  echo "CLEAN, BUT NOT A PUBLISHING CHECK: gitleaks or the forbid list was skipped on purpose."
else
  echo "READY: nothing to fix."
fi
exit $fail
