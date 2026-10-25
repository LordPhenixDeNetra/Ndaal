#!/usr/bin/env bash
# Run every criterion micro-benchmark of the workspace and write the results
# in the "bencher" text format that github-action-benchmark reads (T01.15).
# Usage: bench/micro.sh [output file]   (default bench-output.txt)
set -euo pipefail
cd "$(dirname "$0")/.."

output="${1:-bench-output.txt}"
# A fresh criterion directory: a baseline restored half-way by the CI cache
# (base/ without sample.json) makes criterion print an error in place of the
# result. The history lives in the bench-data branch, not in baselines.
CRITERION_HOME="$(mktemp -d)"
export CRITERION_HOME
# --bench '*': only the criterion targets; libtest harnesses reject the option.
cargo bench --workspace --bench '*' -- --output-format bencher | tee "$output"
