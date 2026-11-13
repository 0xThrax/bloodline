use crate::{Bracket, EntrantId, Match, MatchId, Slot, Source};

/// The largest entrant count for an elimination format. Its bracket size,
/// 2^31, fits in a `u32`, and so do the 2^32 - 1 matches of its full double
/// elimination graph.
pub(crate) const MAX_ELIMINATION_ENTRANTS: u32 = 1 << 31;

/// The largest entrant count for round robin. Its 523,776 matches fit in a
/// `u32` and stay practical to hold in memory.
pub(crate) const MAX_ROUND_ROBIN_ENTRANTS: u32 = 1024;

/// Seed numbers in bracket order, top to bottom, for a bracket of `size` places.
///
/// `size` must be a power of two. Each seed `s` in an order of length `len`
/// splits into the pair `s, 2 * len + 1 - s`, so the two best seeds can meet
/// only in the final.
pub(crate) fn seed_order(size: u32) -> Vec<u32> {
    let mut order = vec![1];
    while (order.len() as u32) < size {
        let next_size = 2 * order.len() as u32 + 1;
        order = order.iter().flat_map(|&s| [s, next_size - s]).collect();
    }
    order
}

/// Builds the single elimination graph for `entrants`, in seed order, with
/// the matches ordered by id. The caller guarantees
/// `2 <= entrants.len() <= MAX_ELIMINATION_ENTRANTS`.
pub(crate) fn single_elimination(entrants: &[EntrantId]) -> Vec<Match> {
    let size = (entrants.len() as u32).next_power_of_two();
    let mut plan = Plan::default();
    winners_bracket(&mut plan, size);
    remove_byes(plan.0, entrants)
}

/// Builds the double elimination graph for `entrants`, in seed order, with
/// the matches ordered by id. The caller guarantees
/// `2 <= entrants.len() <= MAX_ELIMINATION_ENTRANTS`.
pub(crate) fn double_elimination(entrants: &[EntrantId]) -> Vec<Match> {
    let size = (entrants.len() as u32).next_power_of_two();
    let mut plan = Plan::default();
    let winners = winners_bracket(&mut plan, size);
    let losers_final = losers_bracket(&mut plan, size, &winners);
    if let Some(winners_final) = winners.last() {
        grand_final(&mut plan, winners_final.get(0), losers_final);
    }
    remove_byes(plan.0, entrants)
}

/// Builds the round robin schedule for `entrants`, in seed order, with the
/// matches ordered by id. The caller guarantees
/// `2 <= entrants.len() <= MAX_ROUND_ROBIN_ENTRANTS`.
///
/// Uses the circle method. An odd entrant count gets a dummy seed, and a
/// pairing with it is a bye. Seed 1 stays in place while the other seeds
/// rotate one place each round, and seed 1 changes side every round.
pub(crate) fn round_robin(entrants: &[EntrantId]) -> Vec<Match> {
    let count = entrants.len() as u32;
    let places = count + count % 2;
    let mut circle: Vec<u32> = (1..=places).collect();
    let mut matches = Vec::with_capacity((count * (count - 1) / 2) as usize);
    for round in 1..places {
        for position in 0..places / 2 {
            let mut seeds = [
                circle[position as usize],
                circle[(places - 1 - position) as usize],
            ];
            if position == 0 && round % 2 == 0 {
                seeds.reverse();
            }
            if seeds.iter().any(|&seed| seed > count) {
                continue;
            }
            matches.push(Match {
                id: MatchId(matches.len() as u32),
                bracket: Bracket::Group,
                round,
                position,
                slots: seeds.map(|seed| slot(Source::Seed(seed), entrants)),
                result: None,
            });
        }
        circle[1..].rotate_right(1);
    }
    matches
}

/// A match of the full graph, before byes are removed.
///
/// Its `Winner` and `Loser` sources hold indices into the full graph, not
/// final ids.
struct Planned {
    bracket: Bracket,
    round: u32,
    position: u32,
    sources: [Source; 2],
}

/// The full graph, in id order.
#[derive(Default)]
struct Plan(Vec<Planned>);

/// The matches of one round in a [`Plan`], which are contiguous.
#[derive(Clone, Copy)]
struct Round {
    first: u32,
}

impl Round {
    fn get(self, position: u32) -> MatchId {
        MatchId(self.first + position)
    }
}

impl Plan {
    fn push_round(
        &mut self,
        bracket: Bracket,
        round: u32,
        sources: impl IntoIterator<Item = [Source; 2]>,
    ) -> Round {
        let first = self.0.len() as u32;
        for (position, sources) in sources.into_iter().enumerate() {
            self.0.push(Planned {
                bracket,
                round,
                position: position as u32,
                sources,
            });
        }
        Round { first }
    }
}

/// Adds the winners bracket for `size` entrants, a power of two, and returns
/// its rounds in order.
fn winners_bracket(plan: &mut Plan, size: u32) -> Vec<Round> {
    let mut rounds = Vec::new();
    let mut places: Vec<Source> = seed_order(size).into_iter().map(Source::Seed).collect();
    while places.len() > 1 {
        let pairs = places.as_chunks::<2>().0;
        let count = pairs.len() as u32;
        let round = plan.push_round(
            Bracket::Winners,
            rounds.len() as u32 + 1,
            pairs.iter().cloned(),
        );
        places = (0..count).map(|p| Source::Winner(round.get(p))).collect();
        rounds.push(round);
    }
    rounds
}

/// Adds the losers bracket for `size` entrants and returns its final, or
/// `None` if the winners bracket has only one round.
fn losers_bracket(plan: &mut Plan, size: u32, winners: &[Round]) -> Option<Round> {
    let mut last: Option<Round> = None;
    for k in 1..winners.len() as u32 {
        let count = size >> (k + 1);
        let (feeder, source): (Round, fn(MatchId) -> Source) = match last {
            None => (winners[0], Source::Loser),
            Some(previous) => (previous, Source::Winner),
        };
        let odd = plan.push_round(
            Bracket::Losers,
            2 * k - 1,
            (0..count).map(|p| [source(feeder.get(2 * p)), source(feeder.get(2 * p + 1))]),
        );
        let dropping = winners[k as usize];
        // Alternate the order of the entrants that drop in, so that two
        // entrants who just met in the winners bracket do not meet again at once.
        let drop_position = |p: u32| if k % 2 == 1 { count - 1 - p } else { p };
        let even = plan.push_round(
            Bracket::Losers,
            2 * k,
            (0..count).map(|p| {
                [
                    Source::Winner(odd.get(p)),
                    Source::Loser(dropping.get(drop_position(p))),
                ]
            }),
        );
        last = Some(even);
    }
    last
}

/// Adds the grand final and its reset.
fn grand_final(plan: &mut Plan, winners_final: MatchId, losers_final: Option<Round>) {
    let side_b = match losers_final {
        Some(losers_final) => Source::Winner(losers_final.get(0)),
        None => Source::Loser(winners_final),
    };
    let first = plan.push_round(
        Bracket::GrandFinal,
        1,
        [[Source::Winner(winners_final), side_b]],
    );
    plan.push_round(
        Bracket::GrandFinal,
        2,
        [[Source::Loser(first.get(0)), Source::Winner(first.get(0))]],
    );
}

/// Where the winner and the loser of a planned match come from after byes
/// are removed.
struct Outcome {
    winner: Option<Source>,
    loser: Option<Source>,
}

/// Removes every match of `planned` that lacks two entrants, and gives the
/// rest dense ids in order.
///
/// `planned` is in id order, and each match refers only to earlier ones. A
/// removed match passes its one present source on to the slot its winner
/// feeds. The slot its loser feeds gets no entrant.
fn remove_byes(planned: Vec<Planned>, entrants: &[EntrantId]) -> Vec<Match> {
    let count = entrants.len() as u32;
    let mut outcomes: Vec<Outcome> = Vec::with_capacity(planned.len());
    let mut matches = Vec::new();
    for plan in planned {
        let present = plan.sources.map(|source| match source {
            Source::Seed(seed) => (seed <= count).then_some(Source::Seed(seed)),
            Source::Winner(index) => outcomes
                .get(index.0 as usize)
                .and_then(|outcome| outcome.winner.clone()),
            Source::Loser(index) => outcomes
                .get(index.0 as usize)
                .and_then(|outcome| outcome.loser.clone()),
        });
        let outcome = match present {
            [Some(a), Some(b)] => {
                let id = MatchId(matches.len() as u32);
                matches.push(Match {
                    id,
                    bracket: plan.bracket,
                    round: plan.round,
                    position: plan.position,
                    slots: [slot(a, entrants), slot(b, entrants)],
                    result: None,
                });
                Outcome {
                    winner: Some(Source::Winner(id)),
                    loser: Some(Source::Loser(id)),
                }
            }
            [Some(present), None] | [None, Some(present)] => Outcome {
                winner: Some(present),
                loser: None,
            },
            [None, None] => Outcome {
                winner: None,
                loser: None,
            },
        };
        outcomes.push(outcome);
    }
    matches
}

fn slot(source: Source, entrants: &[EntrantId]) -> Slot {
    let entrant = match source {
        Source::Seed(seed) => entrants.get(seed as usize - 1).copied(),
        Source::Winner(_) | Source::Loser(_) => None,
    };
    Slot { source, entrant }
}

#[cfg(test)]
mod tests {
    use super::seed_order;

    #[test]
    fn seed_order_for_small_brackets() {
        assert_eq!(seed_order(1), [1]);
        assert_eq!(seed_order(2), [1, 2]);
        assert_eq!(seed_order(4), [1, 4, 2, 3]);
        assert_eq!(seed_order(8), [1, 8, 4, 5, 2, 7, 3, 6]);
        assert_eq!(
            seed_order(16),
            [1, 16, 8, 9, 4, 13, 5, 12, 2, 15, 7, 10, 3, 14, 6, 11]
        );
    }
}
