#!/usr/bin/env bash
# Run every criterion micro-benchmark of the workspace and write the results
# in the "bencher" text format that github-action-benchmark reads (T01.15).
# Usage: bench/micro.sh [output file]   (default bench-output.txt)
set -euo pipefail
cd "$(dirname "$0")/.."

output="${1:-bench-output.txt}"
cargo bench --workspace -- --output-format bencher | tee "$output"
