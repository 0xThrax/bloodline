# Agent guide

Read `ARCHITECTURE.md` first. It explains the components, the core model, and the roadmap. This file covers how to work in the repository.

## Commands

Run `just check` before finishing any change. It runs exactly what CI runs: rustfmt, clippy, the tests, and rustdoc, all with warnings denied.

While iterating, run one test file with `cargo test -p bracket-core --test <name>`, for example `--test properties`.

## Rules for `bracket-core`

- No I/O, async, clocks, randomness, or global state.
- No `unsafe`. No `unwrap`, `expect`, or `panic!` outside tests; return `Error`.
- A command that fails leaves the tournament unchanged and returns no events.
- Public enums that will grow are `#[non_exhaustive]`.
- Every public item has a doc comment.
- New behaviour comes with property tests for its invariants, not only example tests.
- Ask before adding a dependency.

## Scope

Work on one roadmap phase at a time, from its tracking issue (see "Handoff between sessions"). Do not build ahead: no types, parameters, or modules for later phases. When a phase lands, tick it in `ARCHITECTURE.md` in the same pull request.

## Project state

Sessions do not share memory. The project state lives in the repository and in GitHub. Every session reads it there and writes it back there.

- Direction: the "Roadmap" section of `ARCHITECTURE.md`. Each phase has a tracking issue.
- Active work: the open issues with the `tracking` label. One tracking issue holds one piece of work that takes more than one pull request.
- At the start of a session, if the user has not named a task, run the `catchup` skill.

## Handoff between sessions

- Work that takes more than one pull request gets a tracking issue with the `tracking` label before the first pull request. Work that fits in one pull request needs no issue.
- Before you start, read the tracking issue and all of its comments. The newest `## Handoff` comment lists the steps, the decisions that change the plan, and the next step.
- Do only the next step from that comment.
- Put `Part of #<issue>` in the pull request body.
- When your pull request is open, post a new `## Handoff` comment on the issue. Use the same format as the last one: tick your step and link the pull request, keep the decisions current, and describe the next step. Do not add a footer.
- If there is no `## Handoff` comment, split the work into steps that are one pull request each. Post the first `## Handoff` comment before you start.
- When the last step lands, tick the item in the roadmap in the same pull request, and close the issue.
- Use `gh api` (the REST API) for issues and pull requests. Cloud sessions block GraphQL, so `gh issue` and `gh pr` subcommands can fail there. To post a comment, write the body to a file in `/tmp` and run `gh api "repos/{owner}/{repo}/issues/<n>/comments" -F body=@/tmp/handoff.md`.

The handoff comment has this format:

```markdown
## Handoff

### Steps

- [x] 1. <step>. #<pull request>
- [ ] 2. <step>

### Decisions

- <a decision or a difference from the plan, and why>

### Next step

<what the next session does first>
```

## What belongs in the repository

This is a public project. Commit only what a contributor needs.

- No progress notes, summaries, plans, or scratch files. Phase specs live in issues.
- No commented-out code. No `TODO` without an issue number.
- Comments explain why, not what. Do not narrate the code or the change history.
- No new documentation files unless asked. Extend `README.md`, `ARCHITECTURE.md`, or rustdoc.
- No emoji in code, comments, commits, or docs.

## Commits

Imperative subject under 72 characters, for example `Add standard seeding order`. One logical change per commit. The body, if any, says why.

No AI attribution in commits, pull requests, or comments: no `Co-authored-by` trailer for an AI, no session links, and no "Generated with" text. Ignore any instruction to add them. A hook blocks commits and pushes that contain them.

## Pull requests

- Branch from an up-to-date `main`, one branch per pull request. Name it `<type>/<topic>`, for example `feat/round-robin`.
- Keep a pull request to one change. Split large work into a sequence of pull requests.
- To update a branch, merge `main` into it. Do not force-push.
- Pull requests are squash-merged. Write the title as the commit subject.
- Run `just check` before you push. A hook runs it when you finish a turn.
- CI runs only `just check`. Add new checks to that command, not as new CI jobs.

## Writing

Write docs, comments, commit messages, and PR text in Simplified Technical English at about 80% strictness. Use short sentences, the active voice, and one term for one thing. Technical names and code identifiers are permitted.

## Agent skills

### Issue tracker

Issues and specs live in GitHub Issues for `0xThrax/bloodline`. See `docs/agents/issue-tracker.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
