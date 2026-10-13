#!/usr/bin/env bash
# Lint GitHub Actions workflows with actionlint (and the shellcheck it embeds),
# in a container pinned by digest (CLAUDE.md section 4.12).
set -euo pipefail
cd "$(dirname "$0")/.."

# rhysd/actionlint 1.7.12
IMAGE="rhysd/actionlint@sha256:b1934ee5f1c509618f2508e6eb47ee0d3520686341fec936f3b79331f9315667"
SRC="$(pwd -W 2>/dev/null || pwd)"

MSYS_NO_PATHCONV=1 docker run --rm -v "$SRC:/repo:ro" -w /repo "$IMAGE" -color
