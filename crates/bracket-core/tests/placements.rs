use bracket_core::{
    Bracket, Bronze, Command, EntrantId, Format, Match, MatchResult, Placement, Points, Setup,
    Side, Status, Tournament,
};

/// Creates a tournament of `format` where entrant `s` has seed `s`.
fn create(format: Format, count: u64) -> Tournament {
    let setup = Setup {
        format,
        entrants: (1..=count).map(EntrantId).collect(),
    };
    Tournament::create(setup).unwrap().0
}

/// Reports the first ready match, with the winner `pick` chooses, until no
/// match is ready or `reports` results are in.
fn play(tournament: &mut Tournament, reports: usize, pick: impl Fn(&Match) -> Side) {
    for _ in 0..reports {
        let Some(m) = tournament.matches().iter().find(|m| m.is_ready()) else {
            return;
        };
        let command = Command::ReportResult {
            match_id: m.id(),
            result: MatchResult {
                winner: Some(pick(m)),
                score: None,
            },
        };
        tournament.execute(command).unwrap();
    }
}

fn side_a(_: &Match) -> Side {
    Side::A
}

fn placements(entries: &[(u32, u64)]) -> Vec<Placement> {
    entries
        .iter()
        .map(|&(place, entrant)| Placement {
            place,
            entrant: EntrantId(entrant),
        })
        .collect()
}

fn single(bronze: Bronze) -> Format {
    Format::SingleElimination { bronze }
}

#[test]
fn no_places_before_any_result() {
    for format in [
        single(Bronze::None),
        single(Bronze::Match),
        single(Bronze::Repechage),
        Format::DoubleElimination,
        Format::RoundRobin {
            points: Points::default(),
        },
    ] {
        assert_eq!(create(format, 8).placements(), []);
    }
}

#[test]
fn single_elimination_without_bronze() {
    // Semifinals 1 v 4 and 2 v 3, then the final 1 v 2.
    let mut tournament = create(single(Bronze::None), 4);
    play(&mut tournament, 1, side_a);
    assert_eq!(tournament.placements(), placements(&[(3, 4)]));
    play(&mut tournament, usize::MAX, side_a);
    assert_eq!(
        tournament.placements(),
        placements(&[(1, 1), (2, 2), (3, 4), (3, 3)])
    );
}

#[test]
fn single_elimination_with_2_entrants_has_no_third_place() {
    let mut tournament = create(single(Bronze::None), 2);
    play(&mut tournament, usize::MAX, side_a);
    assert_eq!(tournament.placements(), placements(&[(1, 1), (2, 2)]));
}

#[test]
fn single_elimination_with_3_entrants_has_one_third_place() {
    // Seed 1 has a bye to the final. Seed 2 beats seed 3 in the semifinal.
    let mut tournament = create(single(Bronze::None), 3);
    play(&mut tournament, usize::MAX, side_a);
    assert_eq!(
        tournament.placements(),
        placements(&[(1, 1), (2, 2), (3, 3)])
    );
}

#[test]
fn single_elimination_with_bronze_match() {
    // The semifinal losers 4 and 3 meet in the bronze match.
    let mut tournament = create(single(Bronze::Match), 4);
    play(&mut tournament, 2, side_a);
    assert_eq!(tournament.placements(), []);
    play(&mut tournament, usize::MAX, side_a);
    assert_eq!(
        tournament.placements(),
        placements(&[(1, 1), (2, 2), (3, 4), (4, 3)])
    );
}

#[test]
fn single_elimination_with_repechage() {
    // Quarterfinals 1 v 8, 4 v 5, 2 v 7, 3 v 6; semifinals 1 v 4 and 2 v 3.
    // Repechage 8 v 5 and 7 v 6. Bronze 8 v 3 and 7 v 4.
    let mut tournament = create(single(Bronze::Repechage), 8);
    play(&mut tournament, usize::MAX, side_a);
    assert_eq!(
        tournament.placements(),
        placements(&[(1, 1), (2, 2), (3, 8), (3, 7), (5, 3), (5, 4)])
    );
}

#[test]
fn repechage_with_5_entrants_gives_third_place_without_a_match() {
    // Byes remove the bronze match of the semifinal 1 v 4, so seed 4 takes
    // place 3 when it loses that semifinal. Bronze is 5 v 3.
    let mut tournament = create(single(Bronze::Repechage), 5);
    play(&mut tournament, 2, side_a);
    assert_eq!(tournament.placements(), placements(&[(3, 4)]));
    play(&mut tournament, usize::MAX, side_a);
    assert_eq!(
        tournament.placements(),
        placements(&[(1, 1), (2, 2), (3, 4), (3, 5), (5, 3)])
    );
}

#[test]
fn double_elimination() {
    // Losers final 4 v 2, then the grand final 1 v 4, won by side A.
    let mut tournament = create(Format::DoubleElimination, 4);
    play(&mut tournament, 5, side_a);
    assert_eq!(tournament.placements(), placements(&[(3, 2)]));
    play(&mut tournament, usize::MAX, side_a);
    assert_eq!(
        tournament.placements(),
        placements(&[(1, 1), (2, 4), (3, 2)])
    );
}

#[test]
fn double_elimination_decided_by_the_reset() {
    // Side B wins the grand final, so 1 and 4 meet again in the reset.
    let mut tournament = create(Format::DoubleElimination, 4);
    let pick = |m: &Match| {
        if m.bracket() == Bracket::GrandFinal && m.round() == 1 {
            Side::B
        } else {
            Side::A
        }
    };
    play(&mut tournament, 6, pick);
    assert_eq!(tournament.placements(), placements(&[(3, 2)]));
    play(&mut tournament, usize::MAX, pick);
    assert_eq!(
        tournament.status(),
        Status::Complete {
            champion: EntrantId(1)
        }
    );
    assert_eq!(
        tournament.placements(),
        placements(&[(1, 1), (2, 4), (3, 2)])
    );
}

#[test]
fn double_elimination_with_2_entrants_has_no_third_place() {
    let mut tournament = create(Format::DoubleElimination, 2);
    play(&mut tournament, usize::MAX, side_a);
    assert_eq!(tournament.placements(), placements(&[(1, 1), (2, 2)]));
}

#[test]
fn round_robin_after_the_last_result() {
    // Each entrant wins once, so the seed decides the ranks.
    let mut tournament = create(
        Format::RoundRobin {
            points: Points::default(),
        },
        3,
    );
    play(&mut tournament, 2, side_a);
    assert_eq!(tournament.placements(), []);
    play(&mut tournament, usize::MAX, side_a);
    assert_eq!(
        tournament.placements(),
        placements(&[(1, 1), (2, 2), (3, 3)])
    );
}
