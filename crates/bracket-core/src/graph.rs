use crate::{EntrantId, Match, MatchId, Slot, Source};

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

/// Builds the single elimination graph for `entrants`, in seed order.
///
/// Returns the matches, ordered by id, and the round count. The caller
/// guarantees `2 <= entrants.len() <= MAX_ENTRANTS`.
pub(crate) fn single_elimination(entrants: &[EntrantId]) -> (Vec<Match>, u32) {
    let count = entrants.len() as u32;
    let size = count.next_power_of_two();

    // Each place in the current round holds the source that fills it. In
    // round 1, `None` is a seed above the entrant count.
    let mut places: Vec<Option<Source>> = seed_order(size)
        .into_iter()
        .map(|seed| (seed <= count).then_some(Source::Seed(seed)))
        .collect();
    let mut matches = Vec::new();
    let mut round = 0;

    while places.len() > 1 {
        round += 1;
        places = places
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
            .map(|(position, pair)| match pair {
                [Some(a), Some(b)] => {
                    let id = MatchId(matches.len() as u32);
                    matches.push(Match {
                        id,
                        round,
                        position: position as u32,
                        slots: [slot(a, entrants), slot(b, entrants)],
                        result: None,
                    });
                    Some(Source::Winner(id))
                }
                // A bye: the present seed moves straight to the next round.
                [Some(present), None] | [None, Some(present)] => Some(present.clone()),
                _ => None,
            })
            .collect();
    }

    (matches, round)
}

fn slot(source: &Source, entrants: &[EntrantId]) -> Slot {
    let entrant = match source {
        Source::Seed(seed) => entrants.get(*seed as usize - 1).copied(),
        Source::Winner(_) => None,
    };
    Slot {
        source: source.clone(),
        entrant,
    }
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
