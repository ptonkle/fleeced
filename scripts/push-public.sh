#!/usr/bin/env bash
# Push this commit's files to origin main as one commit on the public history,
# which starts after the gracenote rewrite (R1.521); the local history stays whole.
set -euo pipefail
cd "$(dirname "$0")/.."
tree=$(git rev-parse 'HEAD^{tree}')
msg="Built from $(git rev-parse --short HEAD)"
parent=$(git ls-remote origin refs/heads/main | cut -f1)
if [ -n "$parent" ]; then
  git fetch -q origin main
  commit=$(git commit-tree "$tree" -p "$parent" -m "$msg")
else
  commit=$(git commit-tree "$tree" -m "$msg")
fi
git push -q origin "$commit:refs/heads/main"
echo "pushed $commit"
