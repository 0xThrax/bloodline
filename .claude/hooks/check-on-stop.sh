#!/usr/bin/env bash
# Claude Code Stop hook: the turn cannot end while `just check` fails on
# Rust changes made on this branch. Exit 2 sends stderr back to Claude.
set -u
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

base=$(git merge-base HEAD origin/main 2>/dev/null) || exit 0
paths=(':(glob)**/*.rs' ':(glob)**/Cargo.toml' Cargo.lock rust-toolchain.toml justfile)

# Fingerprint every Rust-relevant change since main, committed or not.
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

if just check 1>&2; then
  echo "$fingerprint" > "$stamp"
  exit 0
fi
echo "just check failed. Fix the errors above before finishing." >&2
exit 2
