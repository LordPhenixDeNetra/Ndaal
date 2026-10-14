#!/usr/bin/env bash
# Run one cargo fuzz target on its versioned corpus (T01.14).
# Usage: fuzz.sh <target> [seconds]   (default 60)
# Linux and macOS: the nightly of fuzz/rust-toolchain.toml and the pinned
# cargo-fuzz. Windows: inside the pinned Rust container (Docker).
set -euo pipefail
cd "$(dirname "$0")/.."

target="${1:?usage: fuzz.sh <target> [seconds]}"
seconds="${2:-60}"
nightly="$(sed -n 's/^channel = "\(.*\)"$/\1/p' fuzz/rust-toolchain.toml)"
fuzz_version="$(awk '$1 == "cargo-fuzz" { print $2 }' scripts/tools.versions)"

run() {
  rustup toolchain install "$nightly" --profile minimal
  cargo install --locked cargo-fuzz --version "$fuzz_version"
  cargo +"$nightly" fuzz run "$target" "fuzz/corpus/$target" -- \
    -max_total_time="$seconds" -rss_limit_mb=2048
}

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    # rust:1.99.0-bookworm, as in check-linux.sh.
    image="rust@sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6"
    src="$(pwd -W)"
    MSYS_NO_PATHCONV=1 docker run --rm -v "$src:/work" -w /work \
      -v ndaal-cargo-registry:/usr/local/cargo/registry \
      "$image" bash scripts/fuzz.sh "$target" "$seconds"
    ;;
  *)
    run
    ;;
esac
