#!/usr/bin/env bash
# Relative links and anchors of every Markdown file (T01.17, CLAUDE.md
# section 11). Rules live in xtask/src/docs/.
set -euo pipefail
cd "$(dirname "$0")/.."
exec cargo run --quiet -p xtask -- links "$@"
