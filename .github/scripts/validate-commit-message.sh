#!/usr/bin/env sh
set -eu

message_file="${1:?commit message file is required}"

allowed_scopes='core|routing|api|backends|ollama|llamacpp|config|health|models|logging|tests|docs|repo|ops|ci'
allowed_types='feat|fix|refactor|perf|test|docs|style|chore|ci'

first_line="$(sed -n '1p' "$message_file")"

if [ -z "$first_line" ]; then
  echo "Commit message header is required." >&2
  exit 1
fi

if printf '%s' "$first_line" | grep -Eq '^Merge '; then
  exit 0
fi

if printf '%s' "$first_line" | grep -Eq '^revert: .+'; then
  if ! grep -Eq '^This reverts commit [0-9a-fA-F]+' "$message_file"; then
    echo "Revert commits must include 'This reverts commit <hash>' in the body." >&2
    exit 1
  fi
else
  if ! printf '%s' "$first_line" | grep -Eq "^(${allowed_types})\\((${allowed_scopes})\\): .+"; then
    echo "Commit header must match type(scope): subject using an approved type and scope." >&2
    exit 1
  fi

  subject="$(printf '%s' "$first_line" | sed -E "s/^(${allowed_types})\\((${allowed_scopes})\\): //")"
  first_subject_char="$(printf '%s' "$subject" | cut -c1)"

  case "$first_subject_char" in
    [A-Z])
      echo "Commit subject must not start with a capital letter." >&2
      exit 1
      ;;
  esac

  case "$subject" in
    *.)
      echo "Commit subject must not end with a period." >&2
      exit 1
      ;;
  esac
fi

while IFS= read -r line || [ -n "$line" ]; do
  line_length="$(printf '%s' "$line" | wc -c | tr -d ' ')"
  if [ "$line_length" -gt 100 ]; then
    echo "Commit message lines must not exceed 100 characters." >&2
    exit 1
  fi
done < "$message_file"
