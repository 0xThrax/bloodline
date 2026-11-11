use std::collections::BTreeSet;

use crate::graph::{self, MAX_ENTRANTS};
use crate::{Error, Event, Format, Match, MatchId, Setup, Side, Status};

/// A tournament: its setup and the current state of every match.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tournament {
    setup: Setup,
    matches: Vec<Match>,
    round_count: u32,
}

impl Tournament {
    /// Creates a tournament from `setup` and returns it with its
    /// [`Event::Created`] event.
    ///
    /// Every slot fed by a seed holds its entrant. Every slot fed by a match
    /// is empty.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TooFewEntrants`] for fewer than two entrants,
    /// [`Error::TooManyEntrants`] for more than 2^31, and
    /// [`Error::DuplicateEntrant`] for the first entrant listed twice.
    pub fn create(setup: Setup) -> Result<(Self, Vec<Event>), Error> {
        validate(&setup)?;
        let (matches, round_count) = match setup.format {
            Format::SingleElimination => graph::single_elimination(&setup.entrants),
        };
        let event = Event::Created {
            setup: setup.clone(),
        };
        let tournament = Self {
            setup,
            matches,
            round_count,
        };
        Ok((tournament, vec![event]))
    }

    /// The setup this tournament was created from.
    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    /// All matches, ordered by id.
    pub fn matches(&self) -> &[Match] {
        &self.matches
    }

    /// The match with this id, if it exists.
    pub fn get(&self, id: MatchId) -> Option<&Match> {
        self.matches.get(usize::try_from(id.0).ok()?)
    }

    /// The number of rounds. The final is in the last round.
    pub fn round_count(&self) -> u32 {
        self.round_count
    }

    /// Whether the tournament has a champion.
    pub fn status(&self) -> Status {
        let champion = self.matches.last().and_then(|last| {
            let side = match last.result.as_ref()?.winner {
                Side::A => 0,
                Side::B => 1,
            };
            last.slots[side].entrant
        });
        match champion {
            Some(champion) => Status::Complete { champion },
            None => Status::InProgress,
        }
    }
}

fn validate(setup: &Setup) -> Result<(), Error> {
    check_entrant_count(setup.entrants.len())?;
    let mut seen = BTreeSet::new();
    match setup
        .entrants
        .iter()
        .find(|&&entrant| !seen.insert(entrant))
    {
        Some(&duplicate) => Err(Error::DuplicateEntrant(duplicate)),
        None => Ok(()),
    }
}

fn check_entrant_count(count: usize) -> Result<(), Error> {
    if count < 2 {
        Err(Error::TooFewEntrants)
    } else if u32::try_from(count).map_or(true, |count| count > MAX_ENTRANTS) {
        Err(Error::TooManyEntrants)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_ENTRANTS, check_entrant_count};
    use crate::Error;

    #[test]
    fn entrant_count_limits() {
        let max = MAX_ENTRANTS as usize;
        assert_eq!(check_entrant_count(1), Err(Error::TooFewEntrants));
        assert_eq!(check_entrant_count(2), Ok(()));
        assert_eq!(check_entrant_count(max), Ok(()));
        assert_eq!(check_entrant_count(max + 1), Err(Error::TooManyEntrants));
    }
}
