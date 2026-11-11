use crate::{EntrantId, MatchId, MatchResult, Setup, Side};

/// A request to change a tournament.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Command {
    /// Record the result of a ready match.
    ReportResult {
        /// The match to report.
        match_id: MatchId,
        /// The result of the match.
        result: MatchResult,
    },
}

/// A change to a tournament. Replaying all events of a tournament rebuilds it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Event {
    /// The tournament was created from this setup.
    Created {
        /// The setup of the tournament.
        setup: Setup,
    },
    /// A match got its result.
    ResultReported {
        /// The match that got the result.
        match_id: MatchId,
        /// The result of the match.
        result: MatchResult,
    },
    /// An entrant moved into a slot.
    SlotFilled {
        /// The match that holds the slot.
        match_id: MatchId,
        /// The side of the slot.
        side: Side,
        /// The entrant now in the slot.
        entrant: EntrantId,
    },
    /// The final got its result.
    Completed {
        /// The winner of the final.
        champion: EntrantId,
    },
}
