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

impl Side {
    pub(crate) fn index(self) -> usize {
        match self {
            Side::A => 0,
            Side::B => 1,
        }
    }
}

/// The structure of a tournament.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Format {
    /// Each match eliminates its loser. The winner of the final is the champion.
    SingleElimination,
    /// Each entrant is eliminated after two losses.
    ///
    /// The losers of winners bracket matches drop into the losers bracket.
    /// The grand final puts the winners bracket champion on side A against
    /// the losers bracket champion on side B. If side B wins, the two meet
    /// again in a reset, and the winner of the reset is the champion.
    DoubleElimination,
    /// Each entrant plays every other entrant once, and every match is in
    /// [`Bracket::Group`]. A match can end in a draw.
    RoundRobin {
        /// The points an entrant earns for each result.
        points: Points,
    },
}

/// The points an entrant earns for a win, a draw, and a loss in round robin.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Points {
    /// The points for a win.
    pub win: u32,
    /// The points for a draw.
    pub draw: u32,
    /// The points for a loss.
    pub loss: u32,
}

impl Default for Points {
    /// Three points for a win, one for a draw, and none for a loss.
    fn default() -> Self {
        Self {
            win: 3,
            draw: 1,
            loss: 0,
        }
    }
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

/// A part of a tournament that holds its own sequence of rounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Bracket {
    /// The main bracket. Every single elimination match is here.
    Winners,
    /// The bracket that losers of winners bracket matches drop into.
    Losers,
    /// The final between the winners and losers brackets.
    GrandFinal,
    /// The group of a round robin. Every round robin match is here.
    Group,
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
    /// The loser of this match.
    Loser(MatchId),
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
    /// The side that won, or `None` for a draw.
    ///
    /// Single and double elimination matches cannot end in a draw. Round robin
    /// matches can.
    pub winner: Option<Side>,
    /// The score as `[A, B]`. The core records it and does not check it against `winner`.
    pub score: Option<[u32; 2]>,
}

/// A match between the entrants of two slots.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Match {
    pub(crate) id: MatchId,
    pub(crate) bracket: Bracket,
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

    /// The bracket this match belongs to.
    pub fn bracket(&self) -> Bracket {
        self.bracket
    }

    /// The 1-based round of this match within its bracket.
    pub fn round(&self) -> u32 {
        self.round
    }

    /// The 0-based position of this match within its round and bracket, from
    /// top to bottom.
    ///
    /// In an elimination format, positions count every match of the full
    /// power-of-two bracket. In round robin, they count every pairing of the
    /// round, including the one with the bye when the entrant count is odd.
    /// Either way, a match removed for a bye leaves a gap.
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

    pub(crate) fn winner(&self) -> Option<EntrantId> {
        self.slots[self.result.as_ref()?.winner?.index()].entrant
    }
}

/// Whether a tournament has a champion.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Status {
    /// No result has ended the tournament yet.
    InProgress,
    /// A result has ended the tournament.
    Complete {
        /// The winner of the match that ended an elimination tournament, or
        /// the entrant with rank 1 once every round robin match has a result.
        champion: EntrantId,
    },
}

/// The place of one entrant in a round robin, from the results reported so far.
///
/// Totals saturate at `u32::MAX`; ranking uses the exact totals.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Standing {
    /// The entrant.
    pub entrant: EntrantId,
    /// The 1-based rank. No two entrants share a rank.
    pub rank: u32,
    /// The matches with a result.
    pub played: u32,
    /// The matches won.
    pub wins: u32,
    /// The matches drawn.
    pub draws: u32,
    /// The matches lost.
    pub losses: u32,
    /// The points earned under the tournament's [`Points`].
    pub points: u32,
    /// The total of this entrant's scores. A result without a score adds 0.
    pub score_for: u32,
    /// The total of the opponents' scores. A result without a score adds 0.
    pub score_against: u32,
}
