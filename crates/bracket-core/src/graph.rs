use crate::{Bracket, EntrantId, Match, MatchId, Slot, Source};

/// The largest entrant count. Its bracket size, 2^31, still fits in a `u32`.
pub(crate) const MAX_ENTRANTS: u32 = 1 << 31;

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
/// `2 <= entrants.len() <= MAX_ENTRANTS`.
pub(crate) fn single_elimination(entrants: &[EntrantId]) -> Vec<Match> {
    let size = (entrants.len() as u32).next_power_of_two();
    remove_byes(winners_bracket(size), entrants)
}

/// A match of the full graph, before byes are removed.
///
/// Its `Winner` sources hold indices into the full graph, not final ids.
struct Planned {
    bracket: Bracket,
    round: u32,
    position: u32,
    sources: [Source; 2],
}

/// The winners bracket for `size` entrants, a power of two, with no byes.
fn winners_bracket(size: u32) -> Vec<Planned> {
    let mut planned: Vec<Planned> = Vec::new();
    let mut places: Vec<Source> = seed_order(size).into_iter().map(Source::Seed).collect();
    let mut round = 0;
    while places.len() > 1 {
        round += 1;
        places = places
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
            .map(|(position, [a, b])| {
                let index = MatchId(planned.len() as u32);
                planned.push(Planned {
                    bracket: Bracket::Winners,
                    round,
                    position: position as u32,
                    sources: [a.clone(), b.clone()],
                });
                Source::Winner(index)
            })
            .collect();
    }
    planned
}

/// Removes every match of `planned` that lacks two entrants, and gives the
/// rest dense ids in order.
///
/// `planned` is in id order, and each match refers only to earlier ones. A
/// removed match passes its one present source on to the slot it fed.
fn remove_byes(planned: Vec<Planned>, entrants: &[EntrantId]) -> Vec<Match> {
    let count = entrants.len() as u32;
    // For each planned match, the source of its winner after byes are removed.
    let mut winners: Vec<Option<Source>> = Vec::with_capacity(planned.len());
    let mut matches = Vec::new();
    for plan in planned {
        let present = plan.sources.map(|source| match source {
            Source::Seed(seed) => (seed <= count).then_some(Source::Seed(seed)),
            Source::Winner(index) => winners.get(index.0 as usize).cloned().flatten(),
        });
        let winner = match present {
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
                Some(Source::Winner(id))
            }
            [Some(present), None] | [None, Some(present)] => Some(present),
            [None, None] => None,
        };
        winners.push(winner);
    }
    matches
}

fn slot(source: Source, entrants: &[EntrantId]) -> Slot {
    let entrant = match source {
        Source::Seed(seed) => entrants.get(seed as usize - 1).copied(),
        Source::Winner(_) => None,
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
