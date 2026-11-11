//! Tournament bracket logic: match graphs, results, and event replay.
//!
//! The crate is pure and synchronous. It performs no I/O and reads no clock or
//! random source, so the same events always rebuild the same tournament.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod command;
mod error;
mod graph;
mod model;
mod tournament;

pub use command::{Command, Event};
pub use error::Error;
pub use model::{
    EntrantId, Format, Match, MatchId, MatchResult, Setup, Side, Slot, Source, Status,
};
pub use tournament::Tournament;
