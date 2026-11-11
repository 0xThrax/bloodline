# Agent guide

Read `ARCHITECTURE.md` first. It explains the components, the core model, and the roadmap. This file covers how to work in the repository.

## Commands

Run all three before finishing any change. CI runs the same ones.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

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

## What belongs in the repository

This is a public project. Commit only what a contributor needs.

- No progress notes, summaries, plans, or scratch files. Phase specs live in issues.
- No commented-out code. No `TODO` without an issue number.
- Comments explain why, not what. Do not narrate the code or the change history.
- No new documentation files unless asked. Extend `README.md`, `ARCHITECTURE.md`, or rustdoc.
- No emoji in code, comments, commits, or docs.

## Commits

Imperative subject under 72 characters, for example `Add standard seeding order`. One logical change per commit. The body, if any, says why.
