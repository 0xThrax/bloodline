use std::collections::BTreeSet;

use crate::graph::{self, MAX_ENTRANTS};
use crate::{
    Bracket, Command, Error, Event, Format, Match, MatchId, MatchResult, Setup, Side, Source,
    Status,
};

/// A tournament: its setup and the current state of every match.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tournament {
    setup: Setup,
    matches: Vec<Match>,
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
        let matches = match setup.format {
            Format::SingleElimination => graph::single_elimination(&setup.entrants),
            Format::DoubleElimination => graph::double_elimination(&setup.entrants),
        };
        let event = Event::Created {
            setup: setup.clone(),
        };
        let tournament = Self { setup, matches };
        Ok((tournament, vec![event]))
    }

    /// Executes `command` and returns the events that describe the change.
    ///
    /// [`Command::ReportResult`] returns [`Event::ResultReported`], then
    /// [`Event::SlotFilled`] for the slot the winner moves into, then
    /// [`Event::SlotFilled`] for the slot the loser moves into. A slot event
    /// is left out if no slot takes that entrant. If the result ends the
    /// tournament, neither entrant moves and [`Event::Completed`] comes last.
    ///
    /// The final ends a single elimination tournament. A double elimination
    /// tournament ends when side A wins the grand final, or else when the
    /// reset gets its result.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownMatch`] if the match does not exist,
    /// [`Error::MatchAlreadyComplete`] if it has a result,
    /// [`Error::MatchNotReady`] if a slot has no entrant yet, and
    /// [`Error::DrawNotAllowed`] if the result is a draw. On an error the
    /// tournament is unchanged.
    pub fn execute(&mut self, command: Command) -> Result<Vec<Event>, Error> {
        let events = match command {
            Command::ReportResult { match_id, result } => self.report_result(match_id, result)?,
        };
        for event in &events {
            self.apply(event);
        }
        Ok(events)
    }

    /// Rebuilds a tournament from its events, oldest first.
    ///
    /// Each reported result runs again as a command on the state that the
    /// events before it produce.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLog`] if the first event is not
    /// [`Event::Created`] with a valid setup, if a reported result is not
    /// valid for the current state, or if a reported result is not followed
    /// by exactly the events that [`Tournament::execute`] returns with it.
    pub fn replay(events: impl IntoIterator<Item = Event>) -> Result<Self, Error> {
        let mut events = events.into_iter();
        let mut tournament = match events.next() {
            Some(Event::Created { setup }) => Self::create(setup).map_err(|_| Error::InvalidLog)?.0,
            _ => return Err(Error::InvalidLog),
        };
        while let Some(event) = events.next() {
            let Event::ResultReported { match_id, result } = event else {
                return Err(Error::InvalidLog);
            };
            let produced = tournament
                .execute(Command::ReportResult { match_id, result })
                .map_err(|_| Error::InvalidLog)?;
            // The first produced event is the `ResultReported` just read.
            for expected in produced.into_iter().skip(1) {
                if events.next() != Some(expected) {
                    return Err(Error::InvalidLog);
                }
            }
        }
        Ok(tournament)
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

    /// The number of rounds in `bracket`, or 0 if the tournament has no
    /// match in it.
    ///
    /// Rounds are numbered as in the full power-of-two bracket, so this is
    /// the round of the last match in `bracket`.
    pub fn round_count(&self, bracket: Bracket) -> u32 {
        self.matches
            .iter()
            .rev()
            .find(|m| m.bracket == bracket)
            .map_or(0, |m| m.round)
    }

    /// Whether the tournament has a champion.
    pub fn status(&self) -> Status {
        // Only the last match, or the grand final just before the reset, can
        // end a tournament.
        self.matches
            .iter()
            .rev()
            .take(2)
            .find(|m| {
                m.result
                    .as_ref()
                    .and_then(|result| result.winner)
                    .is_some_and(|winner| self.ends_tournament(m, winner))
            })
            .and_then(Match::winner)
            .map_or(Status::InProgress, |champion| Status::Complete { champion })
    }

    fn report_result(&self, id: MatchId, result: MatchResult) -> Result<Vec<Event>, Error> {
        let reported = self.get(id).ok_or(Error::UnknownMatch(id))?;
        if reported.result.is_some() {
            return Err(Error::MatchAlreadyComplete(id));
        }
        let [Some(a), Some(b)] = reported.slots.each_ref().map(|slot| slot.entrant) else {
            return Err(Error::MatchNotReady(id));
        };
        let Some(side) = result.winner else {
            return Err(Error::DrawNotAllowed(id));
        };
        let (winner, loser) = match side {
            Side::A => (a, b),
            Side::B => (b, a),
        };
        let ends = self.ends_tournament(reported, side);
        let mut events = vec![Event::ResultReported {
            match_id: id,
            result,
        }];
        if ends {
            events.push(Event::Completed { champion: winner });
            return Ok(events);
        }
        let moves = [(Source::Winner(id), winner), (Source::Loser(id), loser)];
        for (source, entrant) in moves {
            if let Some((match_id, side)) = self.slot_fed_by(&source) {
                events.push(Event::SlotFilled {
                    match_id,
                    side,
                    entrant,
                });
            }
        }
        Ok(events)
    }

    /// Whether a win for `winner` in `reported` ends the tournament.
    ///
    /// Side A of the grand final comes from the winners bracket and has not
    /// lost yet, so a win for side A ends the tournament before the reset.
    fn ends_tournament(&self, reported: &Match, winner: Side) -> bool {
        let is_last = self
            .matches
            .last()
            .is_some_and(|last| last.id == reported.id);
        let is_grand_final = reported.bracket == Bracket::GrandFinal && reported.round == 1;
        is_last || (is_grand_final && winner == Side::A)
    }

    /// The slot that takes the entrant of `source`, if any.
    fn slot_fed_by(&self, source: &Source) -> Option<(MatchId, Side)> {
        let from = match source {
            Source::Winner(id) | Source::Loser(id) => *id,
            Source::Seed(_) => return None,
        };
        // A slot is always fed by an earlier match, so search only later ones.
        let later = self.matches.get(usize::try_from(from.0).ok()? + 1..)?;
        later.iter().find_map(|m| {
            m.slots
                .iter()
                .zip([Side::A, Side::B])
                .find(|(slot, _)| slot.source == *source)
                .map(|(_, side)| (m.id, side))
        })
    }

    /// Applies one event that `report_result` returned.
    fn apply(&mut self, event: &Event) {
        match event {
            Event::ResultReported { match_id, result } => {
                if let Some(reported) = self.get_mut(*match_id) {
                    reported.result = Some(result.clone());
                }
            }
            Event::SlotFilled {
                match_id,
                side,
                entrant,
            } => {
                if let Some(filled) = self.get_mut(*match_id) {
                    filled.slots[side.index()].entrant = Some(*entrant);
                }
            }
            Event::Completed { .. } | Event::Created { .. } => {}
        }
    }

    fn get_mut(&mut self, id: MatchId) -> Option<&mut Match> {
        self.matches.get_mut(usize::try_from(id.0).ok()?)
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
