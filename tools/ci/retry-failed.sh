#!/usr/bin/env bash
# The macOS runner's suite with retries of what failed (macos.yml only, ADR 0066).
#
#   tools/ci/retry-failed.sh [extra cargo test args…]
#
# Runs `cargo test --workspace --no-fail-fast`, then reruns only the test binaries that
# failed — cargo names each one as "to rerun pass `-p X --test Y`" (or `--lib`, `--bin Z`,
# `--doc`) — up to two more times. GitHub's macOS runners are virtual machines whose timers
# and disks miss timing tests now and then; a test that fails three times in a row still
# fails the job, and everything that needed a retry is printed as "FLAKY on macOS VM:" so
# the flakes stay visible. Bash 3.2 (macOS's own) is enough.
set -euo pipefail

log="$(mktemp)"
trap 'rm -f "$log"' EXIT

status=0
cargo test --workspace --no-fail-fast "$@" 2>&1 | tee "$log" || status=$?
if [ "$status" -eq 0 ]; then
  exit 0
fi
# Each failed binary's arguments, one per line (e.g. "-p cantus-rt --test ipc_process").
todo=()
while IFS= read -r line; do
  todo+=("$line")
done < <(sed -n 's/.*to rerun pass `\(.*\)`.*/\1/p' "$log" | sort -u)
if [ "${#todo[@]}" -eq 0 ]; then
  echo "The suite failed without naming a test binary (a build error?): no retry."
  exit "$status"
fi

flaky=()
for attempt in 2 3; do
  still=()
  for args in ${todo[@]+"${todo[@]}"}; do
    echo "::group::retry $attempt: cargo test $args"
    # shellcheck disable=SC2086 # the arguments are cargo's own words
    if cargo test $args "$@"; then
      flaky+=("$args (passed on attempt $attempt)")
    else
      still+=("$args")
    fi
    echo "::endgroup::"
  done
  todo=(${still[@]+"${still[@]}"})
  if [ "${#todo[@]}" -eq 0 ]; then
    break
  fi
done

if [ "${#flaky[@]}" -gt 0 ]; then
  echo "FLAKY on macOS VM:"
  for f in ${flaky[@]+"${flaky[@]}"}; do
    echo "  $f"
    echo "::warning::FLAKY on macOS VM: $f"
  done
fi
if [ "${#todo[@]}" -gt 0 ]; then
  echo "FAILED three times:"
  for f in ${todo[@]+"${todo[@]}"}; do
    echo "  $f"
  done
  exit 1
fi
