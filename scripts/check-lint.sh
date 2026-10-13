#!/usr/bin/env bash
# Proves that every disallowed-methods / disallowed-types entry of clippy.toml
# still matches a real item (ADR-017, T01.04): tests/fixtures/lint-probe calls
# each one, and clippy must report each path. A mistyped `allow-invalid`
# entry would otherwise disable itself silently.
set -euo pipefail
cd "$(dirname "$0")/.."

output="$(cargo clippy --manifest-path tests/fixtures/lint-probe/Cargo.toml \
  --target-dir "${CARGO_TARGET_DIR:-target}/lint-probe" --quiet 2>&1 || true)"

missing=0
while read -r path; do
  # Unix sockets do not exist on Windows: they are checked by the Linux CI.
  case "$path" in
    *::Unix*) [ "$(uname -s)" = Linux ] || [ "$(uname -s)" = Darwin ] || continue ;;
  esac
  if ! grep -qF "\`$path\`" <<< "$output"; then
    echo "check-lint: clippy does not report $path (typo, or missing from the probe)" >&2
    missing=1
  fi
done < <(grep -oE 'path = "[^"]+"' clippy.toml | cut -d'"' -f2)

if [ "$missing" -ne 0 ]; then
  exit 1
fi
echo "check-lint: every disallowed path is enforced"
