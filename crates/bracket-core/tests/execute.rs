use bracket_core::{
    Command, EntrantId, Error, Event, Format, MatchId, MatchResult, Setup, Side, Status, Tournament,
};

/// Entrant ids are `100 + seed`.
fn setup_with(format: Format, count: u64) -> Setup {
    Setup {
        format,
        entrants: (1..=count).map(|seed| EntrantId(100 + seed)).collect(),
    }
}

fn setup(count: u64) -> Setup {
    setup_with(Format::SingleElimination, count)
}

fn report(match_id: u32, winner: Side) -> Command {
    Command::ReportResult {
        match_id: MatchId(match_id),
        result: MatchResult {
            winner: Some(winner),
            score: None,
        },
    }
}

fn result_reported(match_id: u32, winner: Side) -> Event {
    Event::ResultReported {
        match_id: MatchId(match_id),
        result: MatchResult {
            winner: Some(winner),
            score: None,
        },
    }
}

fn slot_filled(match_id: u32, side: Side, entrant: u64) -> Event {
    Event::SlotFilled {
        match_id: MatchId(match_id),
        side,
        entrant: EntrantId(entrant),
    }
}

fn completed(champion: u64) -> Event {
    Event::Completed {
        champion: EntrantId(champion),
    }
}

/// Runs `commands` on a new tournament and returns it with every event.
fn run(count: u64, commands: impl IntoIterator<Item = Command>) -> (Tournament, Vec<Event>) {
    run_setup(setup(count), commands)
}

fn run_setup(
    setup: Setup,
    commands: impl IntoIterator<Item = Command>,
) -> (Tournament, Vec<Event>) {
    let (mut tournament, mut events) = Tournament::create(setup).unwrap();
    for command in commands {
        events.extend(tournament.execute(command).unwrap());
    }
    (tournament, events)
}

#[test]
fn report_moves_the_winner_and_completes_the_final() {
    let (mut tournament, _) = Tournament::create(setup(3)).unwrap();

    let score = Some([1, 3]);
    let events = tournament
        .execute(Command::ReportResult {
            match_id: MatchId(0),
            result: MatchResult {
                winner: Some(Side::B),
                score,
            },
        })
        .unwrap();
    assert_eq!(
        events,
        [
            Event::ResultReported {
                match_id: MatchId(0),
                result: MatchResult {
                    winner: Some(Side::B),
                    score
                },
            },
            slot_filled(1, Side::B, 103),
        ]
    );
    let final_match = tournament.get(MatchId(1)).unwrap();
    assert_eq!(final_match.slots()[1].entrant(), Some(EntrantId(103)));
    assert!(final_match.is_ready());
    assert_eq!(tournament.status(), Status::InProgress);

    let events = tournament.execute(report(1, Side::A)).unwrap();
    assert_eq!(
        events,
        [
            result_reported(1, Side::A),
            Event::Completed {
                champion: EntrantId(101)
            },
        ]
    );
    assert_eq!(
        tournament.status(),
        Status::Complete {
            champion: EntrantId(101)
        }
    );
}

#[test]
fn unknown_match() {
    let (mut tournament, _) = Tournament::create(setup(4)).unwrap();
    let before = tournament.clone();
    assert_eq!(
        tournament.execute(report(3, Side::A)),
        Err(Error::UnknownMatch(MatchId(3)))
    );
    assert_eq!(tournament, before);
}

#[test]
fn match_not_ready() {
    let (mut tournament, _) = Tournament::create(setup(4)).unwrap();
    tournament.execute(report(0, Side::A)).unwrap();
    let before = tournament.clone();
    assert_eq!(
        tournament.execute(report(2, Side::A)),
        Err(Error::MatchNotReady(MatchId(2)))
    );
    assert_eq!(tournament, before);
}

#[test]
fn match_already_complete() {
    let (mut tournament, _) = Tournament::create(setup(4)).unwrap();
    tournament.execute(report(0, Side::A)).unwrap();
    let before = tournament.clone();
    assert_eq!(
        tournament.execute(report(0, Side::B)),
        Err(Error::MatchAlreadyComplete(MatchId(0)))
    );
    assert_eq!(tournament, before);
}

fn draw(match_id: u32) -> Command {
    Command::ReportResult {
        match_id: MatchId(match_id),
        result: MatchResult {
            winner: None,
            score: Some([1, 1]),
        },
    }
}

#[test]
fn draw_not_allowed_in_single_elimination() {
    let (mut tournament, _) = Tournament::create(setup(4)).unwrap();
    let before = tournament.clone();
    assert_eq!(
        tournament.execute(draw(0)),
        Err(Error::DrawNotAllowed(MatchId(0)))
    );
    assert_eq!(tournament, before);
}

#[test]
fn draw_not_allowed_in_double_elimination() {
    let mut tournament = double_at_grand_final();
    let before = tournament.clone();
    assert_eq!(
        tournament.execute(draw(1)),
        Err(Error::DrawNotAllowed(MatchId(1)))
    );
    assert_eq!(tournament, before);
}

#[test]
fn draw_checked_after_readiness() {
    let (mut tournament, _) = Tournament::create(setup(4)).unwrap();
    assert_eq!(
        tournament.execute(draw(2)),
        Err(Error::MatchNotReady(MatchId(2)))
    );
}

#[test]
fn replay_rebuilds_the_tournament() {
    let (tournament, events) = run(4, [report(0, Side::A), report(1, Side::B)]);
    assert_eq!(Tournament::replay(events), Ok(tournament));
}

#[test]
fn invalid_log() {
    let (_, events) = run(3, [report(0, Side::A), report(1, Side::B)]);
    let created = events[0].clone();
    let invalid: Vec<(&str, Vec<Event>)> = vec![
        ("empty", vec![]),
        ("does not start with Created", events[1..].to_vec()),
        ("second Created", vec![created.clone(), created.clone()]),
        (
            "invalid setup",
            vec![Event::Created {
                setup: Setup {
                    format: Format::SingleElimination,
                    entrants: vec![EntrantId(1)],
                },
            }],
        ),
        (
            "result for a match that is not ready",
            vec![created.clone(), result_reported(1, Side::A)],
        ),
        (
            "result for an unknown match",
            vec![created.clone(), result_reported(2, Side::A)],
        ),
        (
            "draw",
            vec![
                created.clone(),
                Event::ResultReported {
                    match_id: MatchId(0),
                    result: MatchResult {
                        winner: None,
                        score: None,
                    },
                },
            ],
        ),
        ("result without its winner moving", events[..2].to_vec()),
        (
            "two results in a row",
            vec![
                created.clone(),
                result_reported(0, Side::A),
                result_reported(1, Side::A),
            ],
        ),
        (
            "slot filled by the loser",
            vec![
                created.clone(),
                result_reported(0, Side::A),
                slot_filled(1, Side::B, 103),
            ],
        ),
        (
            "slot filled without a result",
            vec![created.clone(), slot_filled(1, Side::B, 102)],
        ),
        (
            "seed slot filled",
            vec![
                created.clone(),
                result_reported(0, Side::A),
                slot_filled(1, Side::A, 102),
            ],
        ),
        (
            "Completed for a match that is not the final",
            vec![
                created.clone(),
                result_reported(0, Side::A),
                Event::Completed {
                    champion: EntrantId(102),
                },
            ],
        ),
        (
            "Completed with the wrong champion",
            [
                &events[..4],
                &[Event::Completed {
                    champion: EntrantId(101),
                }],
            ]
            .concat(),
        ),
        (
            "Completed twice",
            [&events[..], &events[events.len() - 1..]].concat(),
        ),
    ];
    for (case, log) in invalid {
        assert_eq!(Tournament::replay(log), Err(Error::InvalidLog), "{case}");
    }
}

/// A two-entrant double elimination tournament after its winners final, which
/// seed 1 won. Match 1 is the grand final and match 2 is the reset.
fn double_at_grand_final() -> Tournament {
    let (mut tournament, _) = Tournament::create(setup_with(Format::DoubleElimination, 2)).unwrap();
    assert_eq!(
        tournament.execute(report(0, Side::A)).unwrap(),
        [
            result_reported(0, Side::A),
            slot_filled(1, Side::A, 101),
            slot_filled(1, Side::B, 102),
        ]
    );
    tournament
}

#[test]
fn side_a_wins_the_grand_final() {
    let mut tournament = double_at_grand_final();
    assert_eq!(
        tournament.execute(report(1, Side::A)).unwrap(),
        [result_reported(1, Side::A), completed(101)]
    );
    assert_eq!(
        tournament.status(),
        Status::Complete {
            champion: EntrantId(101)
        }
    );
    let reset = tournament.get(MatchId(2)).unwrap();
    assert!(!reset.is_ready());
    assert_eq!(
        tournament.execute(report(2, Side::A)),
        Err(Error::MatchNotReady(MatchId(2)))
    );
}

#[test]
fn side_b_wins_the_grand_final_and_the_reset() {
    let mut tournament = double_at_grand_final();
    assert_eq!(
        tournament.execute(report(1, Side::B)).unwrap(),
        [
            result_reported(1, Side::B),
            slot_filled(2, Side::B, 102),
            slot_filled(2, Side::A, 101),
        ]
    );
    assert_eq!(tournament.status(), Status::InProgress);
    assert!(tournament.get(MatchId(2)).unwrap().is_ready());
    assert_eq!(
        tournament.execute(report(2, Side::B)).unwrap(),
        [result_reported(2, Side::B), completed(102)]
    );
    assert_eq!(
        tournament.status(),
        Status::Complete {
            champion: EntrantId(102)
        }
    );
}

#[test]
fn losers_drop_into_the_losers_bracket() {
    let (tournament, events) = run_setup(
        setup_with(Format::DoubleElimination, 4),
        [report(0, Side::B), report(1, Side::A)],
    );
    assert_eq!(
        events[1..],
        [
            result_reported(0, Side::B),
            slot_filled(2, Side::A, 104),
            slot_filled(3, Side::A, 101),
            result_reported(1, Side::A),
            slot_filled(2, Side::B, 102),
            slot_filled(3, Side::B, 103),
        ]
    );
    assert!(tournament.get(MatchId(3)).unwrap().is_ready());
    assert_eq!(Tournament::replay(events), Ok(tournament));
}

#[test]
fn invalid_double_elimination_log() {
    let (_, events) = run_setup(
        setup_with(Format::DoubleElimination, 2),
        [report(0, Side::A), report(1, Side::B), report(2, Side::A)],
    );
    let invalid: Vec<(&str, Vec<Event>)> = vec![
        ("loser does not move", events[..3].to_vec()),
        (
            "loser moves before the winner",
            [&events[..2], &[events[3].clone(), events[2].clone()]].concat(),
        ),
        (
            "grand final won by side B completes",
            [&events[..5], &[completed(102)]].concat(),
        ),
        (
            "extra event after the reset",
            [&events[..], &[slot_filled(2, Side::A, 101)]].concat(),
        ),
    ];
    for (case, log) in invalid {
        assert_eq!(Tournament::replay(log), Err(Error::InvalidLog), "{case}");
    }
}
