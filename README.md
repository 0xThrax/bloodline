# bloodline

A tournament bracket engine. It has three parts:

- `bracket-core`: a pure Rust library that models brackets.
- `bracket-server`: a server that persists brackets and broadcasts changes.
- `web`: a web client that shows brackets live.

Read [ARCHITECTURE.md](ARCHITECTURE.md) for the design and the roadmap.

## Status

Early development. Phase 1, single elimination in `bracket-core`, is in progress. The server and the web client do not exist yet.

## Development

Install [rustup](https://rustup.rs) and [just](https://github.com/casey/just). The `rust-toolchain.toml` file selects the Rust version. Rustup installs it on first use.

Run all checks:

```sh
just check
```

CI runs the same command.

## License

MIT. See [LICENSE](LICENSE).
