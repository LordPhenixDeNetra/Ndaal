#!/usr/bin/env bash
# Build the documentation site (T01.17): mdBook sources generated from the
# documents at the root, then the pinned mdbook (scripts/tools.versions).
# Output: docs/book/html/
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run --quiet -p xtask -- book
mdbook build docs
