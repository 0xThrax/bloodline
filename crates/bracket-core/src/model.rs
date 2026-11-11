/// An entrant, identified by a number the caller assigns. The core treats it as opaque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EntrantId(pub u64);

/// A match, identified by its index in [`Tournament::matches`](crate::Tournament::matches).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchId(pub u32);

/// One of the two sides of a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Side {
    /// The first slot, at index 0.
    A,
    /// The second slot, at index 1.
    B,
}

/// The structure of a tournament.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Format {
    /// Each match eliminates its loser. The winner of the final is the champion.
    SingleElimination,
}

/// Everything needed to create a tournament.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Setup {
    /// The structure of the tournament.
    pub format: Format,
    /// The entrants in seed order: index 0 is seed 1.
    pub entrants: Vec<EntrantId>,
}

/// Where the entrant of a slot comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Source {
    /// The entrant with this 1-based seed number.
    Seed(u32),
    /// The winner of this match.
    Winner(MatchId),
}

/// One side of a match: where its entrant comes from, and the entrant once known.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Slot {
    pub(crate) source: Source,
    pub(crate) entrant: Option<EntrantId>,
}

impl Slot {
    /// Where the entrant of this slot comes from.
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The entrant in this slot, or `None` if its source match has no result yet.
    pub fn entrant(&self) -> Option<EntrantId> {
        self.entrant
    }
}

/// The outcome of a match.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchResult {
    /// The side that won.
    pub winner: Side,
    /// The score as `[A, B]`. The core records it and does not check it against `winner`.
    pub score: Option<[u32; 2]>,
}

/// A match between the entrants of two slots.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Match {
    pub(crate) id: MatchId,
    pub(crate) round: u32,
    pub(crate) position: u32,
    pub(crate) slots: [Slot; 2],
    pub(crate) result: Option<MatchResult>,
}

impl Match {
    /// The id of this match.
    pub fn id(&self) -> MatchId {
        self.id
    }

    /// The 1-based round of this match. The final is in the last round.
    pub fn round(&self) -> u32 {
        self.round
    }

    /// The 0-based position of this match within its round, from top to bottom.
    ///
    /// Positions count every place in the full power-of-two bracket, so a
    /// round 1 pairing that is a bye leaves a gap.
    pub fn position(&self) -> u32 {
        self.position
    }

    /// The two slots of this match. Index 0 is [`Side::A`].
    pub fn slots(&self) -> &[Slot; 2] {
        &self.slots
    }

    /// The result of this match, if it has been reported.
    pub fn result(&self) -> Option<&MatchResult> {
        self.result.as_ref()
    }

    /// Whether both slots hold an entrant and the match has no result.
    pub fn is_ready(&self) -> bool {
        self.result.is_none() && self.slots.iter().all(|slot| slot.entrant.is_some())
    }
}

/// Whether a tournament has a champion.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Status {
    /// The final has no result yet.
    InProgress,
    /// The final has a result.
    Complete {
        /// The winner of the final.
        champion: EntrantId,
    },
}
