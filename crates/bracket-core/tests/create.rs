use bracket_core::{Bracket, EntrantId, Error, Format, MatchId, Setup, Source, Status, Tournament};

fn setup(entrants: impl IntoIterator<Item = u64>) -> Setup {
    Setup {
        format: Format::SingleElimination,
        entrants: entrants.into_iter().map(EntrantId).collect(),
    }
}

/// Entrant ids are `100 + seed`, so a seed slot's entrant is easy to check.
fn create(count: u64) -> Tournament {
    let (tournament, _) = Tournament::create(setup((1..=count).map(|seed| 100 + seed))).unwrap();
    tournament
}

fn seed(seed: u32) -> Source {
    Source::Seed(seed)
}

fn winner(id: u32) -> Source {
    Source::Winner(MatchId(id))
}

/// Checks ids, brackets, rounds, positions, sources, and seed slot entrants.
fn assert_graph(tournament: &Tournament, round_count: u32, expected: &[(u32, u32, [Source; 2])]) {
    assert_eq!(tournament.round_count(Bracket::Winners), round_count);
    assert_eq!(tournament.round_count(Bracket::Losers), 0);
    assert_eq!(tournament.round_count(Bracket::GrandFinal), 0);
    let actual: Vec<_> = tournament
        .matches()
        .iter()
        .enumerate()
        .map(|(index, m)| {
            assert_eq!(m.id(), MatchId(index as u32));
            assert_eq!(m.bracket(), Bracket::Winners);
            assert!(m.result().is_none());
            for slot in m.slots() {
                let entrant = match slot.source() {
                    Source::Seed(s) => Some(EntrantId(100 + u64::from(*s))),
                    _ => None,
                };
                assert_eq!(slot.entrant(), entrant);
            }
            let [a, b] = m.slots();
            (
                m.round(),
                m.position(),
                [a.source().clone(), b.source().clone()],
            )
        })
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn graph_for_2_entrants() {
    assert_graph(&create(2), 1, &[(1, 0, [seed(1), seed(2)])]);
}

#[test]
fn graph_for_3_entrants() {
    assert_graph(
        &create(3),
        2,
        &[(1, 1, [seed(2), seed(3)]), (2, 0, [seed(1), winner(0)])],
    );
}

#[test]
fn graph_for_4_entrants() {
    assert_graph(
        &create(4),
        2,
        &[
            (1, 0, [seed(1), seed(4)]),
            (1, 1, [seed(2), seed(3)]),
            (2, 0, [winner(0), winner(1)]),
        ],
    );
}

#[test]
fn graph_for_5_entrants() {
    assert_graph(
        &create(5),
        3,
        &[
            (1, 1, [seed(4), seed(5)]),
            (2, 0, [seed(1), winner(0)]),
            (2, 1, [seed(2), seed(3)]),
            (3, 0, [winner(1), winner(2)]),
        ],
    );
}

#[test]
fn graph_for_8_entrants() {
    assert_graph(
        &create(8),
        3,
        &[
            (1, 0, [seed(1), seed(8)]),
            (1, 1, [seed(4), seed(5)]),
            (1, 2, [seed(2), seed(7)]),
            (1, 3, [seed(3), seed(6)]),
            (2, 0, [winner(0), winner(1)]),
            (2, 1, [winner(2), winner(3)]),
            (3, 0, [winner(4), winner(5)]),
        ],
    );
}

#[test]
fn create_returns_created_event() {
    let setup = setup([7, 3, 9]);
    let (tournament, events) = Tournament::create(setup.clone()).unwrap();
    assert_eq!(
        events,
        [bracket_core::Event::Created {
            setup: setup.clone()
        }]
    );
    assert_eq!(tournament.setup(), &setup);
    assert_eq!(tournament.status(), Status::InProgress);
}

#[test]
fn get_and_is_ready() {
    let tournament = create(5);
    assert!(tournament.get(MatchId(0)).unwrap().is_ready());
    assert!(tournament.get(MatchId(2)).unwrap().is_ready());
    assert!(!tournament.get(MatchId(1)).unwrap().is_ready());
    assert!(!tournament.get(MatchId(3)).unwrap().is_ready());
    assert_eq!(tournament.get(MatchId(4)), None);
}

#[test]
fn too_few_entrants() {
    assert_eq!(Tournament::create(setup([])), Err(Error::TooFewEntrants));
    assert_eq!(Tournament::create(setup([1])), Err(Error::TooFewEntrants));
}

#[test]
fn duplicate_entrant() {
    assert_eq!(
        Tournament::create(setup([1, 2, 3, 2, 1])),
        Err(Error::DuplicateEntrant(EntrantId(2)))
    );
}
