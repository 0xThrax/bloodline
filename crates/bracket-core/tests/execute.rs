use bracket_core::{
    Command, EntrantId, Error, Event, Format, MatchId, MatchResult, Setup, Side, Status, Tournament,
};

/// Entrant ids are `100 + seed`.
fn setup(count: u64) -> Setup {
    Setup {
        format: Format::SingleElimination,
        entrants: (1..=count).map(|seed| EntrantId(100 + seed)).collect(),
    }
}

fn report(match_id: u32, winner: Side) -> Command {
    Command::ReportResult {
        match_id: MatchId(match_id),
        result: MatchResult {
            winner,
            score: None,
        },
    }
}

fn result_reported(match_id: u32, winner: Side) -> Event {
    Event::ResultReported {
        match_id: MatchId(match_id),
        result: MatchResult {
            winner,
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

/// Runs `commands` on a new tournament and returns it with every event.
fn run(count: u64, commands: impl IntoIterator<Item = Command>) -> (Tournament, Vec<Event>) {
    let (mut tournament, mut events) = Tournament::create(setup(count)).unwrap();
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
                winner: Side::B,
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
                    winner: Side::B,
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
