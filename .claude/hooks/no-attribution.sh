#!/usr/bin/env bash
# Claude Code PreToolUse hook (Bash): block AI attribution in commits and PRs.
# Exit 2 blocks the tool call and sends stderr back to Claude.
set -u
pattern='Co-authored-by:.*(Claude|anthropic)|Claude-Session:|Generated with \[?Claude'

cmd=$(jq -r '.tool_input.command // empty')
[ -z "$cmd" ] && exit 0

# Commit messages and PR text written inline in the command.
if printf '%s' "$cmd" | grep -Eq '(git commit|gh pr (create|edit)|gh issue|gh api)' &&
   printf '%s' "$cmd" | grep -Eiq "$pattern"; then
  echo "Blocked: the command adds AI attribution. Remove it and run the command again." >&2
  exit 2
fi

# Commits made by any route are checked before they leave the machine.
if printf '%s' "$cmd" | grep -Eq '(^|[;&| ])git push'; then
  cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
  base=$(git merge-base HEAD origin/main 2>/dev/null) || exit 0
  bad=$(git log --format='%h %s%n%b' "$base"..HEAD | grep -Eic "$pattern")
  if [ "$bad" -gt 0 ]; then
    echo "Blocked: commits on this branch have AI attribution trailers. Reword them (git rebase) and push again." >&2
    exit 2
  fi
fi
exit 0
