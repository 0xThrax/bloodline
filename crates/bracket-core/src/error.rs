use crate::{EntrantId, MatchId};

/// Why a tournament could not be created, changed, or replayed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Error {
    /// The setup has fewer entrants than its format needs: 2 for every
    /// format, 4 for single elimination with [`Bronze::Match`](crate::Bronze::Match).
    #[error("the setup has fewer entrants than its format needs")]
    TooFewEntrants,
    /// The setup has more entrants than its format allows: 2^31 for single
    /// and double elimination, 1024 for round robin.
    #[error("the setup has more entrants than its format allows")]
    TooManyEntrants,
    /// The setup lists this entrant more than once.
    #[error("{0:?} appears more than once in the setup")]
    DuplicateEntrant(EntrantId),
    /// The tournament has no match with this id.
    #[error("{0:?} does not exist")]
    UnknownMatch(MatchId),
    /// A slot of this match has no entrant yet.
    #[error("{0:?} is waiting for an entrant")]
    MatchNotReady(MatchId),
    /// This match already has a result.
    #[error("{0:?} already has a result")]
    MatchAlreadyComplete(MatchId),
    /// A result for this match is a draw, and the format does not allow draws.
    #[error("{0:?} cannot end in a draw")]
    DrawNotAllowed(MatchId),
    /// The events do not form a valid history of a tournament.
    #[error("the events do not form a valid tournament history")]
    InvalidLog,
}
