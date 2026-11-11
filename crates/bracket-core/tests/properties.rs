use std::collections::BTreeMap;

use bracket_core::{EntrantId, Format, Setup, Source, Tournament};
use proptest::prelude::*;

/// Setups with 2 to 128 distinct entrants in random seed order.
fn setups() -> impl Strategy<Value = Setup> {
    prop::collection::btree_set(any::<u64>(), 2..=128)
        .prop_map(|ids| ids.into_iter().map(EntrantId).collect::<Vec<_>>())
        .prop_shuffle()
        .prop_map(|entrants| Setup {
            format: Format::SingleElimination,
            entrants,
        })
}

fn create(setup: Setup) -> Tournament {
    Tournament::create(setup).unwrap().0
}

/// The round of the match that holds each seed slot, by seed number.
fn seed_rounds(tournament: &Tournament) -> BTreeMap<u32, Vec<u32>> {
    let mut rounds: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for m in tournament.matches() {
        for slot in m.slots() {
            if let Source::Seed(seed) = slot.source() {
                rounds.entry(*seed).or_default().push(m.round());
            }
        }
    }
    rounds
}

proptest! {
    #[test]
    fn graph_has_one_match_fewer_than_entrants(setup in setups()) {
        let count = setup.entrants.len();
        prop_assert_eq!(create(setup).matches().len(), count - 1);
    }

    #[test]
    fn each_seed_is_in_one_slot_with_its_entrant(setup in setups()) {
        let tournament = create(setup.clone());
        let mut seen = BTreeMap::new();
        for m in tournament.matches() {
            for slot in m.slots() {
                if let Source::Seed(seed) = slot.source() {
                    *seen.entry(*seed).or_insert(0) += 1;
                    let entrant = setup.entrants.get(*seed as usize - 1).copied();
                    prop_assert!(entrant.is_some());
                    prop_assert_eq!(slot.entrant(), entrant);
                }
            }
        }
        let count = setup.entrants.len() as u32;
        let expected: BTreeMap<u32, i32> = (1..=count).map(|seed| (seed, 1)).collect();
        prop_assert_eq!(seen, expected);
    }

    #[test]
    fn every_match_but_the_final_feeds_one_later_slot(setup in setups()) {
        let tournament = create(setup);
        let matches = tournament.matches();
        let mut fed = vec![0; matches.len()];
        for m in matches {
            for slot in m.slots() {
                if let Source::Winner(id) = slot.source() {
                    let source = tournament.get(*id);
                    prop_assert!(source.is_some_and(|source| source.round() < m.round()));
                    fed[id.0 as usize] += 1;
                }
            }
        }
        let last = matches.len() - 1;
        prop_assert_eq!(matches[last].round(), tournament.round_count());
        prop_assert_eq!(fed[last], 0);
        prop_assert!(fed[..last].iter().all(|&count| count == 1));
    }

    #[test]
    fn byes_go_to_the_top_seeds(setup in setups()) {
        let count = setup.entrants.len() as u32;
        let byes = count.next_power_of_two() - count;
        for (seed, rounds) in seed_rounds(&create(setup)) {
            let first_round = if seed <= byes { 2 } else { 1 };
            prop_assert_eq!(rounds, vec![first_round], "seed {}", seed);
        }
    }

    #[test]
    fn create_is_deterministic(setup in setups()) {
        prop_assert_eq!(
            Tournament::create(setup.clone()),
            Tournament::create(setup)
        );
    }
}
