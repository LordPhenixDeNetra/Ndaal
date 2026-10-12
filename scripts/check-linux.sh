#!/usr/bin/env bash
# Lint and test the workspace compiled for Linux, from any host (CLAUDE.md section 11).
# On macOS and Windows, code behind #[cfg(target_os = "linux")] is not even compiled
# by a local cargo clippy; this script runs it in the pinned Rust container.
set -euo pipefail
cd "$(dirname "$0")/.."

# rust:1.99.0-bookworm, pinned by digest (CLAUDE.md section 4.12).
IMAGE="rust@sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6"

# Windows path under Git Bash (pwd -W), plain path elsewhere.
SRC="$(pwd -W 2>/dev/null || pwd)"

# Keep Git Bash from rewriting container paths; build output stays in Docker volumes,
# never in the (possibly synced) source folder.
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$SRC:/work" -w /work \
  -v ndaal-cargo-registry:/usr/local/cargo/registry \
  -v ndaal-target-linux:/target -e CARGO_TARGET_DIR=/target \
  "$IMAGE" \
  bash -c "cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
