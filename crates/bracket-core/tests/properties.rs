use std::collections::BTreeMap;

use bracket_core::{
    Bracket, Command, EntrantId, Event, Format, MatchId, MatchResult, Setup, Side, Source, Status,
    Tournament,
};
use proptest::prelude::*;
use proptest::sample::Index;

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

/// One step of a random run: which ready match to report, and its result.
#[derive(Debug, Clone)]
struct Pick {
    ready: Index,
    winner: Side,
    score: Option<[u32; 2]>,
}

/// Setups with enough picks to finish them.
fn runs() -> impl Strategy<Value = (Setup, Vec<Pick>)> {
    let pick = (
        any::<Index>(),
        prop_oneof![Just(Side::A), Just(Side::B)],
        any::<Option<[u32; 2]>>(),
    )
        .prop_map(|(ready, winner, score)| Pick {
            ready,
            winner,
            score,
        });
    (setups(), prop::collection::vec(pick, 127))
}

fn ready_matches(tournament: &Tournament) -> Vec<MatchId> {
    tournament
        .matches()
        .iter()
        .filter(|m| m.is_ready())
        .map(|m| m.id())
        .collect()
}

fn report(match_id: MatchId, winner: Side, score: Option<[u32; 2]>) -> Command {
    Command::ReportResult {
        match_id,
        result: MatchResult { winner, score },
    }
}

/// Reports a ready match for each pick until the tournament completes.
///
/// Calls `check` before each report with the tournament, the events so far,
/// and the command about to run.
fn run(
    setup: Setup,
    picks: &[Pick],
    mut check: impl FnMut(&Tournament, &[Event], &Command) -> Result<(), TestCaseError>,
) -> Result<(Tournament, Vec<Event>), TestCaseError> {
    let (mut tournament, mut events) = Tournament::create(setup).unwrap();
    for pick in picks {
        let ready = ready_matches(&tournament);
        if ready.is_empty() {
            break;
        }
        let command = report(*pick.ready.get(&ready), pick.winner, pick.score);
        check(&tournament, &events, &command)?;
        events.extend(tournament.execute(command).unwrap());
    }
    Ok((tournament, events))
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
        prop_assert!(matches.iter().all(|m| m.bracket() == Bracket::Winners));
        prop_assert_eq!(matches[last].round(), tournament.round_count(Bracket::Winners));
        prop_assert_eq!(tournament.round_count(Bracket::Losers), 0);
        prop_assert_eq!(tournament.round_count(Bracket::GrandFinal), 0);
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

    #[test]
    fn better_seed_always_wins(setup in setups()) {
        let seed_of: BTreeMap<EntrantId, usize> =
            setup.entrants.iter().enumerate().map(|(index, &e)| (e, index + 1)).collect();
        let (mut tournament, _) = Tournament::create(setup.clone()).unwrap();
        while let Some(&id) = ready_matches(&tournament).first() {
            let [a, b] = tournament.get(id).unwrap().slots().clone();
            let seed = |entrant: Option<EntrantId>| seed_of[&entrant.unwrap()];
            let winner = if seed(a.entrant()) < seed(b.entrant()) { Side::A } else { Side::B };
            tournament.execute(report(id, winner, None)).unwrap();
        }
        let final_match = tournament.matches().last().unwrap();
        let finalists = final_match.slots().clone().map(|slot| slot.entrant());
        prop_assert_eq!(finalists, [Some(setup.entrants[0]), Some(setup.entrants[1])]);
        prop_assert_eq!(tournament.status(), Status::Complete { champion: setup.entrants[0] });
    }

    #[test]
    fn completes_after_one_report_per_match((setup, picks) in runs()) {
        let count = setup.entrants.len();
        let mut reports = 0;
        let (tournament, _) = run(setup.clone(), &picks, |tournament, _, _| {
            prop_assert_eq!(tournament.status(), Status::InProgress);
            reports += 1;
            Ok(())
        })?;
        prop_assert_eq!(reports, count - 1);
        let Status::Complete { champion } = tournament.status() else {
            return Err(TestCaseError::fail("no champion"));
        };

        let mut losses: BTreeMap<EntrantId, u32> =
            setup.entrants.iter().map(|&entrant| (entrant, 0)).collect();
        for m in tournament.matches() {
            let loser = match m.result().unwrap().winner {
                Side::A => &m.slots()[1],
                Side::B => &m.slots()[0],
            };
            *losses.get_mut(&loser.entrant().unwrap()).unwrap() += 1;
        }
        for (entrant, lost) in losses {
            prop_assert_eq!(lost, u32::from(entrant != champion), "{:?}", entrant);
        }
    }

    #[test]
    fn replay_matches_the_live_tournament((setup, picks) in runs()) {
        let (tournament, events) = run(setup, &picks, |tournament, events, _| {
            prop_assert_eq!(&Tournament::replay(events.to_vec()), &Ok(tournament.clone()));
            Ok(())
        })?;
        prop_assert_eq!(Tournament::replay(events), Ok(tournament));
    }

    #[test]
    fn failed_command_leaves_the_tournament_unchanged(
        (setup, picks) in runs(),
        targets in prop::collection::vec(any::<Index>(), 127),
    ) {
        let mut targets = targets.into_iter();
        run(setup, &picks, |tournament, _, valid| {
            let unknown = MatchId(tournament.matches().len() as u32);
            let not_ready: Vec<MatchId> = tournament
                .matches()
                .iter()
                .filter(|m| !m.is_ready())
                .map(|m| m.id())
                .chain([unknown])
                .collect();
            let target = *targets.next().unwrap().get(&not_ready);
            let mut after = tournament.clone();
            prop_assert!(after.execute(report(target, Side::A, None)).is_err());
            prop_assert_eq!(&after, tournament);

            after.execute(valid.clone()).unwrap();
            let before = after.clone();
            prop_assert!(after.execute(valid.clone()).is_err());
            prop_assert_eq!(after, before);
            Ok(())
        })?;
    }

    #[test]
    fn ready_matches_are_the_reportable_ones((setup, picks) in runs()) {
        run(setup, &picks, |tournament, _, _| {
            let unknown = MatchId(tournament.matches().len() as u32);
            let ids = tournament.matches().iter().map(|m| m.id()).chain([unknown]);
            for id in ids {
                let ready = tournament.get(id).is_some_and(|m| m.is_ready());
                let accepted = tournament.clone().execute(report(id, Side::B, None)).is_ok();
                prop_assert_eq!(accepted, ready, "{:?}", id);
            }
            Ok(())
        })?;
    }
}
