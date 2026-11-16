use std::collections::BTreeSet;

use crate::graph::{self, MAX_ELIMINATION_ENTRANTS, MAX_ROUND_ROBIN_ENTRANTS};
use crate::standings;
use crate::{
    Bracket, Bronze, Command, EntrantId, Error, Event, Format, Match, MatchId, MatchResult,
    Placement, Setup, Side, Source, Standing, Status,
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
    /// Returns [`Error::TooFewEntrants`] for fewer than two entrants, or
    /// fewer than four with [`Bronze::Match`](crate::Bronze::Match) or five
    /// with [`Bronze::Repechage`](crate::Bronze::Repechage),
    /// [`Error::TooManyEntrants`] for more than 2^31 in an elimination format
    /// or more than 1024 in round robin, and
    /// [`Error::DuplicateEntrant`] for the first entrant listed twice.
    pub fn create(setup: Setup) -> Result<(Self, Vec<Event>), Error> {
        validate(&setup)?;
        let matches = match setup.format {
            Format::SingleElimination { bronze } => {
                graph::single_elimination(&setup.entrants, bronze)
            }
            Format::DoubleElimination => graph::double_elimination(&setup.entrants),
            Format::RoundRobin { .. } => graph::round_robin(&setup.entrants),
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
    /// A single elimination tournament ends with the last result among the
    /// final and the bronze matches, if it has any. A double elimination
    /// tournament ends when side A wins the grand final, or else when the
    /// reset gets its result.
    ///
    /// In round robin, entrants never move and the result can be a draw.
    /// [`Command::ReportResult`] returns [`Event::ResultReported`], then
    /// [`Event::Completed`] with the entrant of rank 1 if no other match is
    /// left without a result.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownMatch`] if the match does not exist,
    /// [`Error::MatchAlreadyComplete`] if it has a result,
    /// [`Error::MatchNotReady`] if a slot has no entrant yet, and
    /// [`Error::DrawNotAllowed`] if the result is a draw in an elimination
    /// format. On an error the tournament is unchanged.
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
    /// This is the round of the last match in `bracket`. Elimination rounds
    /// are numbered as in the full power-of-two bracket, so a round left
    /// without matches by byes still counts.
    pub fn round_count(&self, bracket: Bracket) -> u32 {
        self.matches
            .iter()
            .rev()
            .find(|m| m.bracket == bracket)
            .map_or(0, |m| m.round)
    }

    /// Whether the tournament has a champion.
    ///
    /// A single elimination tournament is complete when the final and every
    /// bronze match have a result, and its champion is the winner of the
    /// final. A round robin is complete when every match has a result, and
    /// its champion is the entrant of rank 1.
    pub fn status(&self) -> Status {
        let champion = match self.setup.format {
            Format::SingleElimination { .. } => {
                let mut deciding = self.final_and_bronze();
                let complete = deciding.clone().all(|m| m.result.is_some());
                deciding.next().filter(|_| complete).and_then(Match::winner)
            }
            Format::DoubleElimination => self.double_elimination_end().and_then(Match::winner),
            Format::RoundRobin { .. } => {
                if self.matches.iter().any(|m| m.result.is_none()) {
                    return Status::InProgress;
                }
                self.rank_one(None)
            }
        };
        champion.map_or(Status::InProgress, |champion| Status::Complete { champion })
    }

    /// The round robin standings, ordered by rank, or `None` for an
    /// elimination format.
    ///
    /// Only reported results count. Entrants are ranked by these keys in
    /// order, the first difference deciding:
    ///
    /// 1. Points, from the tournament's [`Points`](crate::Points).
    /// 2. Head-to-head points: the points from matches between entrants with
    ///    equal points only. This applies once to each group of equal points
    ///    and does not recurse into smaller groups.
    /// 3. Score difference, `score_for - score_against`, over all matches.
    /// 4. Score for.
    /// 5. Wins.
    /// 6. Seed: the better seed ranks higher.
    pub fn standings(&self) -> Option<Vec<Standing>> {
        let Format::RoundRobin { points } = &self.setup.format else {
            return None;
        };
        Some(standings::rank(
            &self.setup.entrants,
            points,
            self.results(),
        ))
    }

    /// The places decided so far, ordered by place, then by the id of the
    /// match that decided each place.
    ///
    /// | Format | Places |
    /// | --- | --- |
    /// | Single elimination, [`Bronze::None`] | 1 final winner, 2 final loser, 3 both semifinal losers |
    /// | Single elimination, [`Bronze::Match`] | 1, 2, 3 bronze winner, 4 bronze loser |
    /// | Single elimination, [`Bronze::Repechage`] | 1, 2, 3 both bronze winners, 5 both bronze losers |
    /// | Double elimination | 1 champion, 2 the other finalist, 3 losers final loser |
    /// | Round robin | every rank, after the last result |
    ///
    /// A place is decided when the match that decides it has a result.
    /// Places without a match are skipped: with two entrants there are no
    /// semifinals, so there is no place 3. With [`Bronze::Repechage`] and 5
    /// entrants, byes remove one bronze match, and the semifinal loser who
    /// would play it takes place 3 when the semifinal has a result.
    pub fn placements(&self) -> Vec<Placement> {
        let mut decided: Vec<(u32, MatchId, EntrantId)> = Vec::new();
        let mut place = |place: u32, m: &Match, entrant: Option<EntrantId>| {
            decided.extend(entrant.map(|entrant| (place, m.id, entrant)));
        };
        match &self.setup.format {
            Format::SingleElimination { bronze } => {
                let mut deciding = self.final_and_bronze();
                if let Some(final_match) = deciding.next() {
                    place(1, final_match, final_match.winner());
                    place(2, final_match, final_match.loser());
                }
                let bronze_loser = match bronze {
                    Bronze::Repechage => 5,
                    _ => 4,
                };
                for m in deciding {
                    place(3, m, m.winner());
                    place(bronze_loser, m, m.loser());
                }
                // A semifinal loser who feeds no later match is out of the
                // tournament: with no bronze match, or with a bronze match
                // that byes removed.
                let semifinal_round = self.round_count(Bracket::Winners).saturating_sub(1);
                for m in self.matches.iter().filter(|m| {
                    m.bracket == Bracket::Winners
                        && m.round == semifinal_round
                        && self.slot_fed_by(&Source::Loser(m.id)).is_none()
                }) {
                    place(3, m, m.loser());
                }
            }
            Format::DoubleElimination => {
                if let Some(end) = self.double_elimination_end() {
                    place(1, end, end.winner());
                    place(2, end, end.loser());
                }
                let losers_final = self.matches.iter().rfind(|m| m.bracket == Bracket::Losers);
                if let Some(losers_final) = losers_final {
                    place(3, losers_final, losers_final.loser());
                }
            }
            Format::RoundRobin { .. } => {
                if self.status() == Status::InProgress {
                    return Vec::new();
                }
                return self
                    .standings()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|standing| Placement {
                        place: standing.rank,
                        entrant: standing.entrant,
                    })
                    .collect();
            }
        }
        decided.sort_by_key(|&(place, id, _)| (place, id));
        decided
            .into_iter()
            .map(|(place, _, entrant)| Placement { place, entrant })
            .collect()
    }

    /// The match whose result ended a double elimination tournament, if any.
    ///
    /// Only the reset, or the grand final just before it, can end it.
    fn double_elimination_end(&self) -> Option<&Match> {
        self.matches.iter().rev().take(2).find(|m| {
            m.result
                .as_ref()
                .and_then(|result| result.winner)
                .is_some_and(|winner| self.ends_tournament(m, winner))
        })
    }

    /// The round robin entrant of rank 1 with every reported result, plus
    /// `pending`, or `None` for an elimination format.
    fn rank_one(&self, pending: Option<(&Match, &MatchResult)>) -> Option<EntrantId> {
        let Format::RoundRobin { points } = &self.setup.format else {
            return None;
        };
        let results = self.results().chain(pending);
        let standings = standings::rank(&self.setup.entrants, points, results);
        standings.first().map(|first| first.entrant)
    }

    /// Every match that has a result, with its result.
    fn results(&self) -> impl Iterator<Item = (&Match, &MatchResult)> {
        self.matches
            .iter()
            .filter_map(|m| Some((m, m.result.as_ref()?)))
    }

    fn report_result(&self, id: MatchId, result: MatchResult) -> Result<Vec<Event>, Error> {
        let reported = self.get(id).ok_or(Error::UnknownMatch(id))?;
        if reported.result.is_some() {
            return Err(Error::MatchAlreadyComplete(id));
        }
        let [Some(a), Some(b)] = reported.slots.each_ref().map(|slot| slot.entrant) else {
            return Err(Error::MatchNotReady(id));
        };
        // Round robin entrants never move, and it ends with its last result.
        if self.is_round_robin() {
            let is_last = self
                .matches
                .iter()
                .all(|m| m.id == id || m.result.is_some());
            let completed = is_last
                .then(|| self.rank_one(Some((reported, &result))))
                .flatten()
                .map(|champion| Event::Completed { champion });
            let reported = Event::ResultReported {
                match_id: id,
                result,
            };
            return Ok([reported].into_iter().chain(completed).collect());
        }
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
            // A bronze match can be the last result, but the final decides
            // the champion, and it already has its result.
            let champion = match reported.bracket {
                Bracket::Bronze => self.final_and_bronze().next().and_then(Match::winner),
                _ => Some(winner),
            };
            events.extend(champion.map(|champion| Event::Completed { champion }));
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

    fn is_round_robin(&self) -> bool {
        matches!(self.setup.format, Format::RoundRobin { .. })
    }

    /// Whether a win for `winner` in `reported` ends the elimination
    /// tournament.
    ///
    /// Single elimination ends with the last result among the final and the
    /// bronze matches. Side A of the grand final comes from the winners
    /// bracket and has not lost yet, so a win for side A ends a double
    /// elimination tournament before the reset.
    fn ends_tournament(&self, reported: &Match, winner: Side) -> bool {
        if matches!(self.setup.format, Format::SingleElimination { .. }) {
            let mut deciding = self.final_and_bronze();
            return deciding.clone().any(|m| m.id == reported.id)
                && deciding.all(|m| m.id == reported.id || m.result.is_some());
        }
        let is_last = self
            .matches
            .last()
            .is_some_and(|last| last.id == reported.id);
        let is_grand_final = reported.bracket == Bracket::GrandFinal && reported.round == 1;
        is_last || (is_grand_final && winner == Side::A)
    }

    /// The final of a single elimination tournament, then its bronze matches.
    ///
    /// The final is the last winners bracket match. The repechage and bronze
    /// matches follow it, and this skips the repechage matches.
    fn final_and_bronze(&self) -> impl Iterator<Item = &Match> + Clone {
        let last_winners = self
            .matches
            .iter()
            .rposition(|m| m.bracket == Bracket::Winners)
            .unwrap_or(0);
        self.matches
            .get(last_winners..)
            .unwrap_or_default()
            .iter()
            .filter(|m| matches!(m.bracket, Bracket::Winners | Bracket::Bronze))
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
    let (min, max) = match setup.format {
        Format::SingleElimination { bronze } => (
            graph::min_single_elimination_entrants(bronze),
            MAX_ELIMINATION_ENTRANTS,
        ),
        Format::DoubleElimination => (2, MAX_ELIMINATION_ENTRANTS),
        Format::RoundRobin { .. } => (2, MAX_ROUND_ROBIN_ENTRANTS),
    };
    check_entrant_count(setup.entrants.len(), min, max)?;
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

fn check_entrant_count(count: usize, min: u32, max: u32) -> Result<(), Error> {
    if u32::try_from(count).is_ok_and(|count| count < min) {
        Err(Error::TooFewEntrants)
    } else if u32::try_from(count).map_or(true, |count| count > max) {
        Err(Error::TooManyEntrants)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_ELIMINATION_ENTRANTS, MAX_ROUND_ROBIN_ENTRANTS, check_entrant_count};
    use crate::Error;

    #[test]
    fn entrant_count_limits() {
        for max in [MAX_ELIMINATION_ENTRANTS, MAX_ROUND_ROBIN_ENTRANTS] {
            let count = max as usize;
            assert_eq!(check_entrant_count(1, 2, max), Err(Error::TooFewEntrants));
            assert_eq!(check_entrant_count(2, 2, max), Ok(()));
            assert_eq!(check_entrant_count(3, 4, max), Err(Error::TooFewEntrants));
            assert_eq!(check_entrant_count(4, 4, max), Ok(()));
            assert_eq!(check_entrant_count(count, 2, max), Ok(()));
            assert_eq!(
                check_entrant_count(count + 1, 2, max),
                Err(Error::TooManyEntrants)
            );
        }
    }
}
