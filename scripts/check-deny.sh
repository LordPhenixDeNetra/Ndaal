#!/usr/bin/env bash
# Dependency checks (T01.02): the workspace must pass cargo deny, and a GPL fixture
# must be rejected, which proves the license policy is actually enforced.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo deny check

# The fixture is its own workspace; reuse the repository policy with --config
# (a global option: it goes before the check subcommand).
fixture_output="$(cargo deny --manifest-path tests/fixtures/deny-gpl/Cargo.toml \
  --config deny.toml check licenses 2>&1 || true)"
if ! grep -q "license is not explicitly allowed" <<< "$fixture_output"; then
  echo "$fixture_output" >&2
  echo "check-deny: the GPL fixture was not rejected; the license policy is not enforced" >&2
  exit 1
fi
echo "check-deny: GPL fixture rejected as expected"
