use std::collections::BTreeSet;

use crate::graph::{self, MAX_ENTRANTS};
use crate::{
    Command, EntrantId, Error, Event, Format, Match, MatchId, MatchResult, Setup, Side, Source,
    Status,
};

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

    /// Executes `command` and returns the events that describe the change.
    ///
    /// [`Command::ReportResult`] returns [`Event::ResultReported`], then
    /// [`Event::SlotFilled`] for the slot the winner moves into, or
    /// [`Event::Completed`] if the match was the final.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownMatch`] if the match does not exist,
    /// [`Error::MatchAlreadyComplete`] if it has a result, and
    /// [`Error::MatchNotReady`] if a slot has no entrant yet. On an error the
    /// tournament is unchanged.
    pub fn execute(&mut self, command: Command) -> Result<Vec<Event>, Error> {
        let events = match command {
            Command::ReportResult { match_id, result } => self.report_result(match_id, result)?,
        };
        for event in &events {
            self.apply(event)?;
        }
        Ok(events)
    }

    /// Rebuilds a tournament from its events, oldest first.
    ///
    /// The events are applied as recorded. Each must be valid for the state
    /// the events before it produce.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLog`] if the first event is not
    /// [`Event::Created`] with a valid setup, if an event cannot apply to the
    /// current state, or if a reported result is not followed by the
    /// [`Event::SlotFilled`] or [`Event::Completed`] that moves its winner.
    pub fn replay(events: impl IntoIterator<Item = Event>) -> Result<Self, Error> {
        let mut events = events.into_iter();
        let mut tournament = match events.next() {
            Some(Event::Created { setup }) => Self::create(setup).map_err(|_| Error::InvalidLog)?.0,
            _ => return Err(Error::InvalidLog),
        };
        // The match whose result was reported and whose winner has not moved yet.
        let mut unmoved: Option<MatchId> = None;
        for event in events {
            let expected = match &event {
                Event::ResultReported { .. } => None,
                Event::SlotFilled { match_id, side, .. } => {
                    tournament.slot_source(*match_id, *side)
                }
                Event::Completed { .. } => tournament.matches.last().map(|last| last.id),
                Event::Created { .. } => return Err(Error::InvalidLog),
            };
            if unmoved != expected {
                return Err(Error::InvalidLog);
            }
            tournament.apply(&event).map_err(|_| Error::InvalidLog)?;
            unmoved = match event {
                Event::ResultReported { match_id, .. } => Some(match_id),
                _ => None,
            };
        }
        match unmoved {
            None => Ok(tournament),
            Some(_) => Err(Error::InvalidLog),
        }
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
        match self.matches.last().and_then(Match::winner) {
            Some(champion) => Status::Complete { champion },
            None => Status::InProgress,
        }
    }

    fn report_result(&self, id: MatchId, result: MatchResult) -> Result<Vec<Event>, Error> {
        let reported = self.get(id).ok_or(Error::UnknownMatch(id))?;
        if reported.result.is_some() {
            return Err(Error::MatchAlreadyComplete(id));
        }
        let winner = reported.slots[result.winner.index()]
            .entrant
            .filter(|_| reported.is_ready())
            .ok_or(Error::MatchNotReady(id))?;
        let moved = match self.winner_slot(reported) {
            Some((match_id, side)) => Event::SlotFilled {
                match_id,
                side,
                entrant: winner,
            },
            None => Event::Completed { champion: winner },
        };
        Ok(vec![
            Event::ResultReported {
                match_id: id,
                result,
            },
            moved,
        ])
    }

    /// The slot the winner of `from` moves into, or `None` for the final.
    fn winner_slot(&self, from: &Match) -> Option<(MatchId, Side)> {
        let key = (from.round.checked_add(1)?, from.position / 2);
        let index = self
            .matches
            .binary_search_by_key(&key, |m| (m.round, m.position))
            .ok()?;
        let side = if from.position.is_multiple_of(2) {
            Side::A
        } else {
            Side::B
        };
        let to = &self.matches[index];
        (to.slots[side.index()].source == Source::Winner(from.id)).then_some((to.id, side))
    }

    /// The match that feeds this slot, if the slot exists and a match feeds it.
    fn slot_source(&self, id: MatchId, side: Side) -> Option<MatchId> {
        match self.get(id)?.slots[side.index()].source {
            Source::Winner(source) => Some(source),
            Source::Seed(_) => None,
        }
    }

    /// Applies one event after `Created`. On an error the tournament is unchanged.
    fn apply(&mut self, event: &Event) -> Result<(), Error> {
        match event {
            Event::ResultReported { match_id, result } => {
                let reported = self.get_mut(*match_id)?;
                if !reported.is_ready() {
                    return Err(Error::InvalidLog);
                }
                reported.result = Some(result.clone());
            }
            Event::SlotFilled {
                match_id,
                side,
                entrant,
            } => {
                let source = self
                    .slot_source(*match_id, *side)
                    .ok_or(Error::InvalidLog)?;
                self.check_winner(source, *entrant)?;
                let slot = &mut self.get_mut(*match_id)?.slots[side.index()];
                if slot.entrant.is_some() {
                    return Err(Error::InvalidLog);
                }
                slot.entrant = Some(*entrant);
            }
            Event::Completed { champion } => {
                let last = self.matches.last().ok_or(Error::InvalidLog)?.id;
                self.check_winner(last, *champion)?;
            }
            Event::Created { .. } => return Err(Error::InvalidLog),
        }
        Ok(())
    }

    fn check_winner(&self, id: MatchId, entrant: EntrantId) -> Result<(), Error> {
        match self.get(id).and_then(Match::winner) {
            Some(winner) if winner == entrant => Ok(()),
            _ => Err(Error::InvalidLog),
        }
    }

    fn get_mut(&mut self, id: MatchId) -> Result<&mut Match, Error> {
        usize::try_from(id.0)
            .ok()
            .and_then(|index| self.matches.get_mut(index))
            .ok_or(Error::InvalidLog)
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
