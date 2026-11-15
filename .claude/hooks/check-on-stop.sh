#!/usr/bin/env bash
# Claude Code Stop hook: the turn cannot end while the repo check fails on
# changes made on this branch. Exit 2 sends stderr back to Claude.
set -u
CHECK_CMD='just check'

input=$(cat)
active=$(printf '%s' "$input" | jq -r '.stop_hook_active // false' 2>/dev/null || echo false)

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
base=$(git merge-base HEAD origin/main 2>/dev/null) || exit 0

# Fingerprint every non-doc change since main, committed or not.
paths=(. ':(exclude,glob)**/*.md' ':(exclude)LICENSE')
fingerprint=$(
  {
    git diff "$base" -- "${paths[@]}"
    git ls-files --others --exclude-standard -z -- "${paths[@]}" | xargs -0r cat
  } | sha1sum | cut -d' ' -f1
)
empty=$(printf '' | sha1sum | cut -d' ' -f1)
[ "$fingerprint" = "$empty" ] && exit 0

# Skip the run if this exact state already passed.
stamp="$(git rev-parse --git-dir)/claude-check-passed"
[ "$(cat "$stamp" 2>/dev/null)" = "$fingerprint" ] && exit 0

# The check tool is missing (common in a fresh cloud container).
tool=${CHECK_CMD%% *}
if ! command -v "$tool" >/dev/null 2>&1; then
  if [ "$active" = "true" ]; then
    echo "Warning: '$tool' is still not installed. The check did not run." >&2
    exit 0
  fi
  echo "'$tool' is not installed, so '$CHECK_CMD' cannot run. Install it (for just: 'cargo install just --locked'), then finish again." >&2
  exit 2
fi

if bash -c "$CHECK_CMD" 1>&2; then
  echo "$fingerprint" > "$stamp"
  exit 0
fi
echo "'$CHECK_CMD' failed. Fix the errors above before finishing." >&2
exit 2
