use std::cmp::Reverse;

use crate::{EntrantId, Match, MatchResult, Points, Side, Source, Standing};

/// The running totals of one entrant. Sums are `u64` so that no total can
/// overflow while ranking: an entrant plays at most 1023 matches.
#[derive(Default)]
struct Tally {
    wins: u32,
    draws: u32,
    losses: u32,
    points: u64,
    head_to_head: u64,
    score_for: u64,
    score_against: u64,
}

/// One reported match: the 0-based seed of each side and the result.
struct Played<'a> {
    seeds: [usize; 2],
    result: &'a MatchResult,
}

impl Played<'_> {
    /// The points that the entrant on `side` earned.
    fn earned(&self, side: Side, points: &Points) -> u64 {
        let earned = match self.result.winner {
            None => points.draw,
            Some(winner) if winner == side => points.win,
            Some(_) => points.loss,
        };
        u64::from(earned)
    }
}

/// Ranks `entrants`, in seed order, by the round robin `results` reported so
/// far. Every slot of a round robin match has a seed source.
pub(crate) fn rank<'a>(
    entrants: &[EntrantId],
    points: &Points,
    results: impl IntoIterator<Item = (&'a Match, &'a MatchResult)>,
) -> Vec<Standing> {
    let played: Vec<Played<'a>> = results
        .into_iter()
        .filter_map(|(m, result)| {
            let seeds = m.slots.each_ref().map(|slot| match slot.source {
                Source::Seed(seed) => usize::try_from(seed).ok()?.checked_sub(1),
                _ => None,
            });
            match seeds {
                [Some(a), Some(b)] if a < entrants.len() && b < entrants.len() => Some(Played {
                    seeds: [a, b],
                    result,
                }),
                _ => None,
            }
        })
        .collect();

    let mut tallies: Vec<Tally> = entrants.iter().map(|_| Tally::default()).collect();
    for game in &played {
        let score = game.result.score.unwrap_or([0, 0]);
        for (index, side) in [Side::A, Side::B].into_iter().enumerate() {
            let tally = &mut tallies[game.seeds[index]];
            match game.result.winner {
                None => tally.draws += 1,
                Some(winner) if winner == side => tally.wins += 1,
                Some(_) => tally.losses += 1,
            }
            tally.points += game.earned(side, points);
            tally.score_for += u64::from(score[index]);
            tally.score_against += u64::from(score[1 - index]);
        }
    }
    // Head-to-head counts only matches inside a group of equal points, and
    // the groups are fixed by the totals above, so it is applied once.
    for game in &played {
        let [a, b] = game.seeds;
        if tallies[a].points == tallies[b].points {
            tallies[a].head_to_head += game.earned(Side::A, points);
            tallies[b].head_to_head += game.earned(Side::B, points);
        }
    }

    let mut order: Vec<usize> = (0..entrants.len()).collect();
    order.sort_by_key(|&seed| {
        let tally = &tallies[seed];
        let difference = i128::from(tally.score_for) - i128::from(tally.score_against);
        (
            Reverse(tally.points),
            Reverse(tally.head_to_head),
            Reverse(difference),
            Reverse(tally.score_for),
            Reverse(tally.wins),
            seed,
        )
    });
    order
        .into_iter()
        .zip(1..)
        .map(|(seed, rank)| {
            let tally = &tallies[seed];
            Standing {
                entrant: entrants[seed],
                rank,
                played: tally.wins + tally.draws + tally.losses,
                wins: tally.wins,
                draws: tally.draws,
                losses: tally.losses,
                points: saturate(tally.points),
                score_for: saturate(tally.score_for),
                score_against: saturate(tally.score_against),
            }
        })
        .collect()
}

fn saturate(total: u64) -> u32 {
    u32::try_from(total).unwrap_or(u32::MAX)
}
