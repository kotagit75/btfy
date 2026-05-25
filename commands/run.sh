#!/usr/bin/env bash
set -euo pipefail

$@ &
p1=$!

cargo run --release -- -m &
p2=$!

cleanup() {
  kill "$p1" "$p2" 2>/dev/null || true
}
trap cleanup EXIT

wait -n "$p1" "$p2"
