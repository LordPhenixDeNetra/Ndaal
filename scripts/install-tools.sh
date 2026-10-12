#!/usr/bin/env bash
# Install exactly the tool versions listed in scripts/tools.versions (CLAUDE.md section 4.12).
# Idempotent: a tool already installed at the pinned version is skipped.
set -euo pipefail
cd "$(dirname "$0")/.."

while read -r name version; do
  case "$name" in '' | '#'*) continue ;; esac
  installed="$(cargo install --list | awk -v n="$name" '$1 == n { print $2 }')"
  if [ "$installed" = "v$version:" ]; then
    echo "$name $version already installed"
    continue
  fi
  cargo install --locked "$name" --version "$version"
done < scripts/tools.versions
