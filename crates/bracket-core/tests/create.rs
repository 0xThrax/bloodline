use bracket_core::{
    Bracket, Bronze, EntrantId, Error, Format, MatchId, Points, Setup, Source, Status, Tournament,
};

fn setup(entrants: impl IntoIterator<Item = u64>) -> Setup {
    Setup {
        format: Format::SingleElimination {
            bronze: Bronze::None,
        },
        entrants: entrants.into_iter().map(EntrantId).collect(),
    }
}

/// Entrant ids are `100 + seed`, so a seed slot's entrant is easy to check.
fn create_with(format: Format, count: u64) -> Tournament {
    let entrants = (1..=count).map(|seed| EntrantId(100 + seed)).collect();
    let (tournament, _) = Tournament::create(Setup { format, entrants }).unwrap();
    tournament
}

fn create(count: u64) -> Tournament {
    create_with(
        Format::SingleElimination {
            bronze: Bronze::None,
        },
        count,
    )
}

fn seed(seed: u32) -> Source {
    Source::Seed(seed)
}

fn winner(id: u32) -> Source {
    Source::Winner(MatchId(id))
}

fn loser(id: u32) -> Source {
    Source::Loser(MatchId(id))
}

/// The bracket, round, position, and sources of each match.
///
/// Checks ids, missing results, and seed slot entrants on the way.
fn graph(tournament: &Tournament) -> Vec<(Bracket, u32, u32, [Source; 2])> {
    tournament
        .matches()
        .iter()
        .enumerate()
        .map(|(index, m)| {
            assert_eq!(m.id(), MatchId(index as u32));
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
                m.bracket(),
                m.round(),
                m.position(),
                [a.source().clone(), b.source().clone()],
            )
        })
        .collect()
}

/// Checks a single elimination graph, where every match is in the winners bracket.
fn assert_graph(tournament: &Tournament, round_count: u32, expected: &[(u32, u32, [Source; 2])]) {
    assert_eq!(tournament.round_count(Bracket::Winners), round_count);
    assert_eq!(tournament.round_count(Bracket::Losers), 0);
    assert_eq!(tournament.round_count(Bracket::GrandFinal), 0);
    let expected: Vec<_> = expected
        .iter()
        .map(|(round, position, sources)| (Bracket::Winners, *round, *position, sources.clone()))
        .collect();
    assert_eq!(graph(tournament), expected);
}

/// Checks a double elimination graph and its round counts for the winners
/// bracket, the losers bracket, and the grand final.
fn assert_double_graph(
    count: u64,
    round_counts: [u32; 3],
    expected: &[(Bracket, u32, u32, [Source; 2])],
) {
    let tournament = create_with(Format::DoubleElimination, count);
    let brackets = [Bracket::Winners, Bracket::Losers, Bracket::GrandFinal];
    assert_eq!(brackets.map(|b| tournament.round_count(b)), round_counts);
    assert_eq!(graph(&tournament), expected);
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

const W: Bracket = Bracket::Winners;
const L: Bracket = Bracket::Losers;
const GF: Bracket = Bracket::GrandFinal;

#[test]
fn double_graph_for_2_entrants() {
    assert_double_graph(
        2,
        [1, 0, 2],
        &[
            (W, 1, 0, [seed(1), seed(2)]),
            (GF, 1, 0, [winner(0), loser(0)]),
            (GF, 2, 0, [loser(1), winner(1)]),
        ],
    );
}

#[test]
fn double_graph_for_3_entrants() {
    assert_double_graph(
        3,
        [2, 2, 2],
        &[
            (W, 1, 1, [seed(2), seed(3)]),
            (W, 2, 0, [seed(1), winner(0)]),
            (L, 2, 0, [loser(0), loser(1)]),
            (GF, 1, 0, [winner(1), winner(2)]),
            (GF, 2, 0, [loser(3), winner(3)]),
        ],
    );
}

#[test]
fn double_graph_for_4_entrants() {
    assert_double_graph(
        4,
        [2, 2, 2],
        &[
            (W, 1, 0, [seed(1), seed(4)]),
            (W, 1, 1, [seed(2), seed(3)]),
            (W, 2, 0, [winner(0), winner(1)]),
            (L, 1, 0, [loser(0), loser(1)]),
            (L, 2, 0, [winner(3), loser(2)]),
            (GF, 1, 0, [winner(2), winner(4)]),
            (GF, 2, 0, [loser(5), winner(5)]),
        ],
    );
}

#[test]
fn double_graph_for_5_entrants() {
    assert_double_graph(
        5,
        [3, 4, 2],
        &[
            (W, 1, 1, [seed(4), seed(5)]),
            (W, 2, 0, [seed(1), winner(0)]),
            (W, 2, 1, [seed(2), seed(3)]),
            (W, 3, 0, [winner(1), winner(2)]),
            (L, 2, 0, [loser(0), loser(2)]),
            (L, 3, 0, [winner(4), loser(1)]),
            (L, 4, 0, [winner(5), loser(3)]),
            (GF, 1, 0, [winner(3), winner(6)]),
            (GF, 2, 0, [loser(7), winner(7)]),
        ],
    );
}

#[test]
fn double_graph_for_8_entrants() {
    assert_double_graph(
        8,
        [3, 4, 2],
        &[
            (W, 1, 0, [seed(1), seed(8)]),
            (W, 1, 1, [seed(4), seed(5)]),
            (W, 1, 2, [seed(2), seed(7)]),
            (W, 1, 3, [seed(3), seed(6)]),
            (W, 2, 0, [winner(0), winner(1)]),
            (W, 2, 1, [winner(2), winner(3)]),
            (W, 3, 0, [winner(4), winner(5)]),
            (L, 1, 0, [loser(0), loser(1)]),
            (L, 1, 1, [loser(2), loser(3)]),
            (L, 2, 0, [winner(7), loser(5)]),
            (L, 2, 1, [winner(8), loser(4)]),
            (L, 3, 0, [winner(9), winner(10)]),
            (L, 4, 0, [winner(11), loser(6)]),
            (GF, 1, 0, [winner(6), winner(12)]),
            (GF, 2, 0, [loser(13), winner(13)]),
        ],
    );
}

/// Checks a round robin schedule, given as the round, position, and seeds of
/// each match.
fn assert_round_robin(count: u64, round_count: u32, expected: &[(u32, u32, [u32; 2])]) {
    let tournament = create_with(round_robin(), count);
    assert_eq!(tournament.round_count(Bracket::Group), round_count);
    assert_eq!(tournament.round_count(Bracket::Winners), 0);
    assert!(tournament.matches().iter().all(|m| m.is_ready()));
    let expected: Vec<_> = expected
        .iter()
        .map(|&(round, position, [a, b])| (Bracket::Group, round, position, [seed(a), seed(b)]))
        .collect();
    assert_eq!(graph(&tournament), expected);
}

fn round_robin() -> Format {
    Format::RoundRobin {
        points: Points::default(),
    }
}

#[test]
fn round_robin_for_2_entrants() {
    assert_round_robin(2, 1, &[(1, 0, [1, 2])]);
}

#[test]
fn round_robin_for_3_entrants() {
    assert_round_robin(3, 3, &[(1, 1, [2, 3]), (2, 0, [3, 1]), (3, 0, [1, 2])]);
}

#[test]
fn round_robin_for_4_entrants() {
    assert_round_robin(
        4,
        3,
        &[
            (1, 0, [1, 4]),
            (1, 1, [2, 3]),
            (2, 0, [3, 1]),
            (2, 1, [4, 2]),
            (3, 0, [1, 2]),
            (3, 1, [3, 4]),
        ],
    );
}

#[test]
fn round_robin_for_5_entrants() {
    assert_round_robin(
        5,
        5,
        &[
            (1, 1, [2, 5]),
            (1, 2, [3, 4]),
            (2, 0, [5, 1]),
            (2, 2, [2, 3]),
            (3, 0, [1, 4]),
            (3, 1, [5, 3]),
            (4, 0, [3, 1]),
            (4, 1, [4, 2]),
            (5, 0, [1, 2]),
            (5, 2, [4, 5]),
        ],
    );
}

#[test]
fn default_points() {
    assert_eq!(
        Points::default(),
        Points {
            win: 3,
            draw: 1,
            loss: 0
        }
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

#[test]
fn too_many_round_robin_entrants() {
    let setup = Setup {
        format: round_robin(),
        entrants: (0..1025).map(EntrantId).collect(),
    };
    assert_eq!(Tournament::create(setup), Err(Error::TooManyEntrants));
}

fn bronze_match() -> Format {
    Format::SingleElimination {
        bronze: Bronze::Match,
    }
}

/// Checks a single elimination graph with a bronze match: `winners` in the
/// winners bracket, then one bronze match between the semifinal losers.
fn assert_bronze_graph(
    count: u64,
    round_count: u32,
    winners: &[(u32, u32, [Source; 2])],
    semifinals: [u32; 2],
) {
    let tournament = create_with(bronze_match(), count);
    assert_eq!(tournament.round_count(Bracket::Winners), round_count);
    assert_eq!(tournament.round_count(Bracket::Bronze), 1);
    let mut expected: Vec<_> = winners
        .iter()
        .map(|(round, position, sources)| (Bracket::Winners, *round, *position, sources.clone()))
        .collect();
    expected.push((Bracket::Bronze, 1, 0, semifinals.map(loser)));
    assert_eq!(graph(&tournament), expected);
}

#[test]
fn bronze_graph_for_4_entrants() {
    assert_bronze_graph(
        4,
        2,
        &[
            (1, 0, [seed(1), seed(4)]),
            (1, 1, [seed(2), seed(3)]),
            (2, 0, [winner(0), winner(1)]),
        ],
        [0, 1],
    );
}

#[test]
fn bronze_graph_for_8_entrants() {
    assert_bronze_graph(
        8,
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
        [4, 5],
    );
}

#[test]
fn bronze_match_needs_4_entrants() {
    let setup = |count: u64| Setup {
        format: bronze_match(),
        entrants: (1..=count).map(EntrantId).collect(),
    };
    assert_eq!(Tournament::create(setup(3)), Err(Error::TooFewEntrants));
    assert!(Tournament::create(setup(4)).is_ok());
}
