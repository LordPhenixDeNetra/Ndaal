#!/usr/bin/env bash
# Install exactly the tool versions listed in scripts/tools.versions (CLAUDE.md section 4.12).
# Usage: install-tools.sh [tool...]   (no argument: every listed tool)
# Idempotent: a tool already installed at the pinned version is skipped.
set -euo pipefail
cd "$(dirname "$0")/.."

# No arrays: "${array[@]}" on an empty array fails under set -u in bash 3.2 (macOS).
wanted=" $* "
for tool in "$@"; do
  if ! grep -q "^$tool " scripts/tools.versions; then
    echo "install-tools: $tool is not listed in scripts/tools.versions" >&2
    exit 2
  fi
done

while read -r name version; do
  case "$name" in '' | '#'*) continue ;; esac
  if [ "$#" -gt 0 ] && [[ "$wanted" != *" $name "* ]]; then
    continue
  fi
  installed="$(cargo install --list | awk -v n="$name" '$1 == n { print $2 }')"
  if [ "$installed" = "v$version:" ]; then
    echo "$name $version already installed"
    continue
  fi
  cargo install --locked "$name" --version "$version"
done < scripts/tools.versions
