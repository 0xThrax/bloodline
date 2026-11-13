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

Work on one roadmap phase at a time, from its tracking issue. Do not build ahead: no types, parameters, or modules for later phases. When a phase lands, tick it in `ARCHITECTURE.md` in the same pull request.

## Handoff between sessions

A phase can take more than one session and more than one pull request. The tracking issue holds the state between sessions.

- Before you start, read the tracking issue and all of its comments. The newest `## Handoff` comment lists the steps, the decisions that change the spec, and the next step.
- Do only the next step from that comment.
- When your pull request is open, post a new `## Handoff` comment on the issue. Use the same format as the last one: tick your step and link the pull request, keep the decisions current, and describe the next step.
- If there is no `## Handoff` comment, split the phase into steps that are one pull request each. Post the first `## Handoff` comment before you start.

## What belongs in the repository

This is a public project. Commit only what a contributor needs.

- No progress notes, summaries, plans, or scratch files. Phase specs live in issues.
- No commented-out code. No `TODO` without an issue number.
- Comments explain why, not what. Do not narrate the code or the change history.
- No new documentation files unless asked. Extend `README.md`, `ARCHITECTURE.md`, or rustdoc.
- No emoji in code, comments, commits, or docs.

## Commits

Imperative subject under 72 characters, for example `Add standard seeding order`. One logical change per commit. The body, if any, says why.
