#!/usr/bin/env sh
set -eu

branch_name="${1:-}"

if [ -z "$branch_name" ]; then
  branch_name="$(git rev-parse --abbrev-ref HEAD)"
fi

case "$branch_name" in
  HEAD)
    exit 0
    ;;
  main|master)
    echo "Direct work on '$branch_name' is not allowed." >&2
    echo "Create an issue branch named issue/<number>-short-description." >&2
    exit 1
    ;;
esac

if ! printf '%s' "$branch_name" | grep -Eq '^issue/[0-9]+-[a-z0-9][a-z0-9-]*$'; then
  echo "Invalid branch name: $branch_name" >&2
  echo "Expected format: issue/<number>-short-description" >&2
  exit 1
fi
