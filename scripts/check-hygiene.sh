#!/usr/bin/env bash
# Repository hygiene: emoji, non-ASCII, whitespace, line endings, file length,
# forbid(unsafe_code) (CLAUDE.md sections 4.3, 4.6, 4.7). Rules live in xtask/.
set -euo pipefail
cd "$(dirname "$0")/.."
exec cargo run --quiet -p xtask -- hygiene "$@"
