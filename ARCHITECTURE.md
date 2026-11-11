# Architecture

A tournament bracket engine: a pure Rust library that models brackets, a server that persists and broadcasts them, and a web client that renders them live.

## Components

| Path | Role | Status |
| --- | --- | --- |
| `crates/bracket-core` | Bracket logic. Pure, synchronous, no I/O. Published to crates.io. | In progress |
| `crates/bracket-server` | Axum HTTP and WebSocket server. Persists events to SQLite. | Planned |
| `web/` | React and TypeScript client. Renders brackets as SVG. | Planned |

Dependencies point one way: `web` talks to `bracket-server`, and `bracket-server` depends on `bracket-core`. `bracket-core` depends on neither and must stay usable on its own.

## Core model

**Match graph.** A tournament is a flat list of matches. Each match has two slots, and each slot names where its entrant comes from: a seed, the winner of another match, or the loser of another match. Every elimination format is a different way of generating this graph. Once the graph exists, reporting results and advancing entrants works the same for all of them.

**Commands and events.** Callers change a tournament by executing a command. The core validates it and returns the events that describe what changed. Events are the only record of state: replaying a tournament's events from the start rebuilds it exactly.

**Determinism.** The core never reads a clock, generates randomness, or performs I/O. The same events always produce the same state. Anything non-deterministic, such as shuffling entrants, happens in the caller before the command is built.

## Data flow

1. A client sends a command over HTTP or WebSocket.
2. The server loads the tournament and executes the command against the core.
3. The core returns events, or an error that leaves the tournament unchanged.
4. The server appends the events to SQLite in one transaction, each with a per-tournament sequence number.
5. The server broadcasts the events to every client watching that tournament.
6. A reconnecting client sends the last sequence number it saw and receives the events it missed.

On startup the server rebuilds each tournament by replaying its events.

## Roadmap

Each phase lands with its tests passing before the next begins.

- [ ] 1. Single elimination: workspace, CI, match graph, seeding, byes, results, replay
- [ ] 2. Double elimination: losers bracket, grand final with reset
- [ ] 3. Round robin: scheduling, standings, tiebreakers
- [ ] 4. Combat sports formats: divisions, bronze match, repechage, same-team separation
- [ ] 5. Server: REST API, SQLite event log, replay on startup
- [ ] 6. Live updates: WebSocket rooms, sequence numbers, resync on reconnect
- [ ] 7. Web client: bracket view, score entry, standings
- [ ] 8. Release 0.1: crates.io, Docker image, hosted demo

Not scheduled: organizer authentication, Swiss, group stage into bracket.
