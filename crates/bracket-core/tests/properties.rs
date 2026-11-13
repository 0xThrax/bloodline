use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use bracket_core::{
    Bracket, Command, EntrantId, Error, Event, Format, MatchId, MatchResult, Points, Setup, Side,
    Source, Status, Tournament,
};
use proptest::prelude::*;
use proptest::sample::Index;

const SINGLE: Format = Format::SingleElimination;
const DOUBLE: Format = Format::DoubleElimination;

fn round_robin() -> Format {
    Format::RoundRobin {
        points: Points::default(),
    }
}

/// The entrant counts that tests of `format` cover.
fn sizes(format: &Format) -> RangeInclusive<usize> {
    match format {
        Format::RoundRobin { .. } => 2..=32,
        _ => 2..=128,
    }
}

/// The most reports any tournament of `format` from [`setups`] needs.
fn max_reports(format: &Format) -> usize {
    let max = *sizes(format).end();
    match format {
        Format::RoundRobin { .. } => max * (max - 1) / 2,
        _ => 2 * max - 1,
    }
}

/// Setups of `format` with distinct entrants in random seed order.
fn setups(format: Format) -> impl Strategy<Value = Setup> {
    prop::collection::btree_set(any::<u64>(), sizes(&format))
        .prop_map(|ids| ids.into_iter().map(EntrantId).collect::<Vec<_>>())
        .prop_shuffle()
        .prop_map(move |entrants| Setup {
            format: format.clone(),
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
    winner: Option<Side>,
    score: Option<[u32; 2]>,
}

/// Setups of `format` with enough picks to finish them. Picks are draws
/// only where `format` allows them.
fn runs(format: Format) -> impl Strategy<Value = (Setup, Vec<Pick>)> {
    let winner = match format {
        Format::RoundRobin { .. } => {
            prop_oneof![Just(Some(Side::A)), Just(Some(Side::B)), Just(None)].boxed()
        }
        _ => prop_oneof![Just(Some(Side::A)), Just(Some(Side::B))].boxed(),
    };
    let reports = max_reports(&format);
    let pick =
        (any::<Index>(), winner, any::<Option<[u32; 2]>>()).prop_map(|(ready, winner, score)| {
            Pick {
                ready,
                winner,
                score,
            }
        });
    (setups(format), prop::collection::vec(pick, reports))
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
        result: MatchResult {
            winner: Some(winner),
            score,
        },
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
        let command = Command::ReportResult {
            match_id: *pick.ready.get(&ready),
            result: MatchResult {
                winner: pick.winner,
                score: pick.score,
            },
        };
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

/// Checks that each seed is in exactly one slot, in the winners bracket,
/// holding its entrant.
fn check_seed_slots(setup: Setup) -> Result<(), TestCaseError> {
    let tournament = create(setup.clone());
    let mut seen = BTreeMap::new();
    for m in tournament.matches() {
        for slot in m.slots() {
            if let Source::Seed(seed) = slot.source() {
                *seen.entry(*seed).or_insert(0) += 1;
                prop_assert_eq!(m.bracket(), Bracket::Winners);
                let entrant = setup.entrants.get(*seed as usize - 1).copied();
                prop_assert!(entrant.is_some());
                prop_assert_eq!(slot.entrant(), entrant);
            }
        }
    }
    let count = setup.entrants.len() as u32;
    let expected: BTreeMap<u32, i32> = (1..=count).map(|seed| (seed, 1)).collect();
    prop_assert_eq!(seen, expected);
    Ok(())
}

fn check_byes(setup: Setup) -> Result<(), TestCaseError> {
    let count = setup.entrants.len() as u32;
    let byes = count.next_power_of_two() - count;
    for (seed, rounds) in seed_rounds(&create(setup)) {
        let first_round = if seed <= byes { 2 } else { 1 };
        prop_assert_eq!(rounds, vec![first_round], "seed {}", seed);
    }
    Ok(())
}

fn check_deterministic(setup: Setup) -> Result<(), TestCaseError> {
    prop_assert_eq!(Tournament::create(setup.clone()), Tournament::create(setup));
    Ok(())
}

/// Reports every ready match with the better seed as the winner, until the
/// tournament completes.
fn better_seed_wins_every_match(setup: &Setup) -> Tournament {
    let seed_of: BTreeMap<EntrantId, usize> = setup
        .entrants
        .iter()
        .enumerate()
        .map(|(index, &e)| (e, index + 1))
        .collect();
    let (mut tournament, _) = Tournament::create(setup.clone()).unwrap();
    while let Some(&id) = ready_matches(&tournament).first() {
        let [a, b] = tournament.get(id).unwrap().slots().clone();
        let seed = |entrant: Option<EntrantId>| seed_of[&entrant.unwrap()];
        let winner = if seed(a.entrant()) < seed(b.entrant()) {
            Side::A
        } else {
            Side::B
        };
        tournament.execute(report(id, winner, None)).unwrap();
    }
    tournament
}

/// Runs to completion and returns the champion and the losses of each entrant.
///
/// Also checks that the tournament is in progress before each report, and
/// complete after the last.
fn losses_after_run(
    setup: Setup,
    picks: &[Pick],
) -> Result<(Tournament, EntrantId, BTreeMap<EntrantId, u32>), TestCaseError> {
    let mut losses: BTreeMap<EntrantId, u32> =
        setup.entrants.iter().map(|&entrant| (entrant, 0)).collect();
    let (tournament, _) = run(setup, picks, |tournament, _, _| {
        prop_assert_eq!(tournament.status(), Status::InProgress);
        Ok(())
    })?;
    let Status::Complete { champion } = tournament.status() else {
        return Err(TestCaseError::fail("no champion"));
    };
    prop_assert!(ready_matches(&tournament).is_empty());
    for m in tournament.matches() {
        let Some(result) = m.result() else { continue };
        let loser = match result.winner.unwrap() {
            Side::A => &m.slots()[1],
            Side::B => &m.slots()[0],
        };
        *losses.get_mut(&loser.entrant().unwrap()).unwrap() += 1;
    }
    Ok((tournament, champion, losses))
}

fn reported_count(tournament: &Tournament) -> usize {
    tournament
        .matches()
        .iter()
        .filter(|m| m.result().is_some())
        .count()
}

fn check_replay(setup: Setup, picks: &[Pick]) -> Result<(), TestCaseError> {
    let (tournament, events) = run(setup, picks, |tournament, events, _| {
        prop_assert_eq!(
            &Tournament::replay(events.to_vec()),
            &Ok(tournament.clone())
        );
        Ok(())
    })?;
    prop_assert_eq!(Tournament::replay(events), Ok(tournament));
    Ok(())
}

fn check_failed_command(
    setup: Setup,
    picks: &[Pick],
    targets: Vec<Index>,
) -> Result<(), TestCaseError> {
    let mut targets = targets.into_iter();
    run(setup, picks, |tournament, _, valid| {
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

        if !matches!(tournament.setup().format, Format::RoundRobin { .. }) {
            let Command::ReportResult { match_id, .. } = valid else {
                return Err(TestCaseError::fail("not a report"));
            };
            let draw = Command::ReportResult {
                match_id: *match_id,
                result: MatchResult {
                    winner: None,
                    score: None,
                },
            };
            prop_assert_eq!(after.execute(draw), Err(Error::DrawNotAllowed(*match_id)));
            prop_assert_eq!(&after, tournament);
        }

        after.execute(valid.clone()).unwrap();
        let before = after.clone();
        prop_assert!(after.execute(valid.clone()).is_err());
        prop_assert_eq!(after, before);
        Ok(())
    })?;
    Ok(())
}

fn check_ready_matches(setup: Setup, picks: &[Pick]) -> Result<(), TestCaseError> {
    run(setup, picks, |tournament, _, _| {
        let unknown = MatchId(tournament.matches().len() as u32);
        let ids = tournament.matches().iter().map(|m| m.id()).chain([unknown]);
        // Every report goes to one copy. A report changes only its own match
        // and later ones, so in reverse id order each match is checked in the
        // state of `tournament`, without a new copy per accepted report.
        let mut copy = tournament.clone();
        for id in ids.rev() {
            let ready = copy.get(id).is_some_and(|m| m.is_ready());
            let accepted = copy.execute(report(id, Side::B, None)).is_ok();
            prop_assert_eq!(accepted, ready, "{:?}", id);
        }
        Ok(())
    })?;
    Ok(())
}

fn targets(format: &Format) -> impl Strategy<Value = Vec<Index>> {
    prop::collection::vec(any::<Index>(), max_reports(format))
}

/// The two seeds of a round robin match.
fn seeds(m: &bracket_core::Match) -> Result<[u32; 2], TestCaseError> {
    let [a, b] = m.slots();
    match (a.source(), b.source()) {
        (Source::Seed(a), Source::Seed(b)) => Ok([*a, *b]),
        _ => Err(TestCaseError::fail("a round robin slot without a seed")),
    }
}

proptest! {
    #[test]
    fn graph_has_one_match_fewer_than_entrants(setup in setups(SINGLE)) {
        let count = setup.entrants.len();
        prop_assert_eq!(create(setup).matches().len(), count - 1);
    }

    #[test]
    fn double_graph_has_two_matches_per_entrant_but_one(setup in setups(DOUBLE)) {
        let count = setup.entrants.len();
        prop_assert_eq!(create(setup).matches().len(), 2 * count - 1);
    }

    #[test]
    fn each_seed_is_in_one_slot_with_its_entrant(setup in setups(SINGLE)) {
        check_seed_slots(setup)?;
    }

    #[test]
    fn double_each_seed_is_in_one_slot_with_its_entrant(setup in setups(DOUBLE)) {
        check_seed_slots(setup)?;
    }

    #[test]
    fn every_match_but_the_final_feeds_one_later_slot(setup in setups(SINGLE)) {
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
    fn double_matches_feed_the_expected_slots(setup in setups(DOUBLE)) {
        let tournament = create(setup);
        let matches = tournament.matches();
        // For each match, the brackets and rounds of the slots its winner and its loser feed.
        let mut winner_feeds = vec![Vec::new(); matches.len()];
        let mut loser_feeds = vec![Vec::new(); matches.len()];
        for m in matches {
            for slot in m.slots() {
                let (feeds, id) = match slot.source() {
                    Source::Winner(id) => (&mut winner_feeds, id),
                    Source::Loser(id) => (&mut loser_feeds, id),
                    _ => continue,
                };
                prop_assert!(id.0 < m.id().0, "{:?} feeds {:?}", id, m.id());
                feeds[id.0 as usize].push((m.bracket(), m.round()));
            }
        }
        let reset = matches.len() - 1;
        let grand_final = reset - 1;
        prop_assert_eq!(
            (matches[grand_final].bracket(), matches[grand_final].round()),
            (Bracket::GrandFinal, 1)
        );
        prop_assert_eq!(
            (matches[reset].bracket(), matches[reset].round()),
            (Bracket::GrandFinal, 2)
        );
        let reset_slot = vec![(Bracket::GrandFinal, 2)];
        prop_assert_eq!(&winner_feeds[grand_final], &reset_slot);
        prop_assert_eq!(&loser_feeds[grand_final], &reset_slot);
        prop_assert!(winner_feeds[reset].is_empty());
        prop_assert!(loser_feeds[reset].is_empty());
        for m in &matches[..grand_final] {
            let index = m.id().0 as usize;
            prop_assert_eq!(winner_feeds[index].len(), 1, "{:?}", m.id());
            match m.bracket() {
                Bracket::Winners => {
                    prop_assert_eq!(loser_feeds[index].len(), 1, "{:?}", m.id());
                    prop_assert!(loser_feeds[index][0].0 != Bracket::Winners);
                }
                Bracket::Losers => prop_assert!(loser_feeds[index].is_empty()),
                _ => return Err(TestCaseError::fail("grand final before the end")),
            }
        }
    }

    #[test]
    fn byes_go_to_the_top_seeds(setup in setups(SINGLE)) {
        check_byes(setup)?;
    }

    #[test]
    fn double_byes_go_to_the_top_seeds(setup in setups(DOUBLE)) {
        check_byes(setup)?;
    }

    #[test]
    fn create_is_deterministic(setup in setups(SINGLE)) {
        check_deterministic(setup)?;
    }

    #[test]
    fn double_create_is_deterministic(setup in setups(DOUBLE)) {
        check_deterministic(setup)?;
    }

    #[test]
    fn better_seed_always_wins(setup in setups(SINGLE)) {
        let tournament = better_seed_wins_every_match(&setup);
        let final_match = tournament.matches().last().unwrap();
        let finalists = final_match.slots().clone().map(|slot| slot.entrant());
        prop_assert_eq!(finalists, [Some(setup.entrants[0]), Some(setup.entrants[1])]);
        prop_assert_eq!(tournament.status(), Status::Complete { champion: setup.entrants[0] });
    }

    #[test]
    fn double_better_seed_always_wins(setup in setups(DOUBLE)) {
        let tournament = better_seed_wins_every_match(&setup);
        let [.., grand_final, reset] = tournament.matches() else {
            return Err(TestCaseError::fail("no grand final"));
        };
        let finalists = grand_final.slots().clone().map(|slot| slot.entrant());
        prop_assert_eq!(finalists, [Some(setup.entrants[0]), Some(setup.entrants[1])]);
        prop_assert!(reset.result().is_none());
        prop_assert!(!reset.is_ready());
        prop_assert_eq!(tournament.status(), Status::Complete { champion: setup.entrants[0] });
    }

    #[test]
    fn completes_after_one_report_per_match((setup, picks) in runs(SINGLE)) {
        let count = setup.entrants.len();
        let (tournament, champion, losses) = losses_after_run(setup, &picks)?;
        prop_assert_eq!(reported_count(&tournament), count - 1);
        for (entrant, lost) in losses {
            prop_assert_eq!(lost, u32::from(entrant != champion), "{:?}", entrant);
        }
    }

    #[test]
    fn double_completes_when_every_entrant_but_one_lost_twice(
        (setup, picks) in runs(DOUBLE),
    ) {
        let count = setup.entrants.len();
        let (tournament, champion, losses) = losses_after_run(setup, &picks)?;
        let [.., grand_final, _] = tournament.matches() else {
            return Err(TestCaseError::fail("no grand final"));
        };
        let reports = match grand_final.result().and_then(|result| result.winner) {
            Some(Side::A) => 2 * count - 2,
            _ => 2 * count - 1,
        };
        prop_assert_eq!(reported_count(&tournament), reports);
        for (entrant, lost) in losses {
            if entrant == champion {
                prop_assert!(lost <= 1, "{:?}", entrant);
            } else {
                prop_assert_eq!(lost, 2, "{:?}", entrant);
            }
        }
    }

    #[test]
    fn replay_matches_the_live_tournament((setup, picks) in runs(SINGLE)) {
        check_replay(setup, &picks)?;
    }

    #[test]
    fn double_replay_matches_the_live_tournament((setup, picks) in runs(DOUBLE)) {
        check_replay(setup, &picks)?;
    }

    #[test]
    fn failed_command_leaves_the_tournament_unchanged(
        (setup, picks) in runs(SINGLE),
        targets in targets(&SINGLE),
    ) {
        check_failed_command(setup, &picks, targets)?;
    }

    #[test]
    fn double_failed_command_leaves_the_tournament_unchanged(
        (setup, picks) in runs(DOUBLE),
        targets in targets(&DOUBLE),
    ) {
        check_failed_command(setup, &picks, targets)?;
    }

    #[test]
    fn ready_matches_are_the_reportable_ones((setup, picks) in runs(SINGLE)) {
        check_ready_matches(setup, &picks)?;
    }

    #[test]
    fn double_ready_matches_are_the_reportable_ones((setup, picks) in runs(DOUBLE)) {
        check_ready_matches(setup, &picks)?;
    }

    #[test]
    fn round_robin_pairs_meet_once(setup in setups(round_robin())) {
        let count = setup.entrants.len() as u32;
        let tournament = create(setup);
        prop_assert_eq!(tournament.matches().len() as u32, count * (count - 1) / 2);
        let mut pairs = BTreeSet::new();
        for m in tournament.matches() {
            prop_assert_eq!(m.bracket(), Bracket::Group);
            let [a, b] = seeds(m)?;
            prop_assert!(a != b && a <= count && b <= count, "{:?}", m.id());
            prop_assert!(pairs.insert((a.min(b), a.max(b))), "{:?}", m.id());
        }
    }

    #[test]
    fn round_robin_rounds_and_byes(setup in setups(round_robin())) {
        let count = setup.entrants.len() as u32;
        let places = count + count % 2;
        let tournament = create(setup);
        prop_assert_eq!(tournament.round_count(Bracket::Group), places - 1);
        let mut rounds_played: BTreeMap<u32, u32> = BTreeMap::new();
        for round in 1..places {
            let mut seen = BTreeSet::new();
            for m in tournament.matches().iter().filter(|m| m.round() == round) {
                for seed in seeds(m)? {
                    prop_assert!(seen.insert(seed), "seed {} twice in round {}", seed, round);
                    *rounds_played.entry(seed).or_default() += 1;
                }
            }
        }
        // An entrant misses only the round of its bye, if the count is odd.
        let expected: BTreeMap<u32, u32> =
            (1..=count).map(|seed| (seed, count - 1)).collect();
        prop_assert_eq!(rounds_played, expected);
    }

    #[test]
    fn round_robin_create_is_deterministic(setup in setups(round_robin())) {
        check_deterministic(setup)?;
    }

    #[test]
    fn round_robin_replay_matches_the_live_tournament((setup, picks) in runs(round_robin())) {
        check_replay(setup, &picks)?;
    }

    #[test]
    fn round_robin_failed_command_leaves_the_tournament_unchanged(
        (setup, picks) in runs(round_robin()),
        targets in targets(&round_robin()),
    ) {
        check_failed_command(setup, &picks, targets)?;
    }

    #[test]
    fn round_robin_ready_matches_are_the_reportable_ones((setup, picks) in runs(round_robin())) {
        check_ready_matches(setup, &picks)?;
    }
}
