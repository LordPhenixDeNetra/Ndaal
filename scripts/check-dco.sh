#!/usr/bin/env bash
# Developer Certificate of Origin check (T01.16, CONTRIBUTING.md).
# Usage: check-dco.sh <base> <head>
# Every non-merge commit in base..head must carry a "Signed-off-by:" line whose
# address is the commit author's. Dependabot commits are accepted unsigned.
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <base> <head>" >&2
  exit 2
fi
base="$1"
head="$2"

# Assign first: in "for x in $(cmd)" a failing git (invalid or unfetched range)
# would be ignored by set -e and the check would pass with zero commits.
commits="$(git rev-list --no-merges "$base..$head")"

failed=0
for sha in $commits; do
  author_name="$(git show -s --format='%an' "$sha")"
  author_email="$(git show -s --format='%ae' "$sha")"
  subject="$(git show -s --format='%s' "$sha")"
  short="$(git rev-parse --short "$sha")"

  if [ "$author_name" = "dependabot[bot]" ]; then
    continue
  fi

  signoffs="$(git show -s --format='%B' "$sha" | grep '^Signed-off-by: ' || true)"
  if [ -z "$signoffs" ]; then
    echo "$short $subject: missing Signed-off-by (use git commit -s)"
    failed=1
  elif ! grep -qF "<$author_email>" <<< "$signoffs"; then
    echo "$short $subject: sign-off does not match author <$author_email>"
    failed=1
  fi
done

if [ "$failed" -ne 0 ]; then
  echo "check-dco: unsigned commits found; see CONTRIBUTING.md" >&2
  exit 1
fi
echo "check-dco: every commit is signed off"
