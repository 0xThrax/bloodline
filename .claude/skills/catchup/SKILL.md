---
name: catchup
description: Report where the project stands and what to do next. Use at the start of a session when the user has not named a task, or when the user asks "where were we", "what's next", "catch me up", or for the project status.
---

# Catch up

Build a short status report from the repository and GitHub. This skill only reads. Do not change files, branches, issues, or pull requests. Do not start work until the user picks a task.

## Gather

Use `gh api` (the REST API). Cloud sessions block GraphQL, so `gh issue list`, `gh pr list`, and `gh pr view` can fail there. `gh api` fills `{owner}` and `{repo}` from the git remote.

1. Direction. Read the roadmap that the "Project state" section of the agent guide names. If it is long, read its headings and the unchecked items.
2. Tracking issues (active work):

   ```bash
   gh api "repos/{owner}/{repo}/issues?labels=tracking&state=open&per_page=30" \
     --jq '.[] | "#\(.number) \(.title)"'
   ```

3. For each tracking issue, the newest handoff:

   ```bash
   gh api "repos/{owner}/{repo}/issues/<n>/comments?per_page=100" \
     --jq '[.[] | select(.body | startswith("## Handoff"))] | last | .body'
   ```

4. Other open issues:

   ```bash
   gh api "repos/{owner}/{repo}/issues?state=open&per_page=50" \
     --jq '.[] | select(.pull_request | not) | "#\(.number) \(.title) [\([.labels[].name] | join(","))]"'
   ```

5. Open pull requests:

   ```bash
   gh api "repos/{owner}/{repo}/pulls?state=open" \
     --jq '.[] | "#\(.number) \(.updated_at[:10]) \(.head.ref) \(.title)"'
   ```

6. Recently merged pull requests:

   ```bash
   gh api "repos/{owner}/{repo}/pulls?state=closed&sort=updated&direction=desc&per_page=20" \
     --jq '[.[] | select(.merged_at)] | sort_by(.merged_at) | reverse | .[:5][] | "#\(.number) \(.merged_at[:10]) \(.title)"'
   ```

   Read the body of the newest one or two with `gh api "repos/{owner}/{repo}/pulls/<n>" --jq .body`. Bodies often name follow-up work.

7. Local state: the current branch, `git status --short`, and `git log --oneline -5`.

## Report

Keep it under 25 lines. Use these headings:

- **Direction:** one or two sentences from the roadmap.
- **Last done:** the newest merged pull requests, one line each.
- **In flight:** open pull requests and tracking issues, with the step each one is on.
- **Next:** the next step from the newest handoff. If there is no handoff, the first unchecked roadmap item. Name the issue.
- **Loose ends:** only real ones. Examples: an open pull request with no update for 7 days, follow-up work that is only in a merged pull request body, a roadmap that does not agree with the issues, uncommitted changes.

End with one question: start the next step, or work on something else?

## When the record is missing

If follow-up work is only in a merged pull request body or in the roadmap, and no tracking issue holds it, say so under **Loose ends** and offer to create a tracking issue. Do not create it until the user agrees.
