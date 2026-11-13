use bracket_core::{
    Command, EntrantId, Event, Format, MatchResult, Points, Setup, Side, Source, Standing, Status,
    Tournament,
};

/// Entrant ids are `100 + seed`.
fn create(format: Format, count: u64) -> Tournament {
    let entrants = (1..=count).map(|seed| EntrantId(100 + seed)).collect();
    Tournament::create(Setup { format, entrants }).unwrap().0
}

fn round_robin(count: u64) -> Tournament {
    create(
        Format::RoundRobin {
            points: Points::default(),
        },
        count,
    )
}

/// Reports the match between seeds `first` and `second` with `score` as
/// `[first, second]`. The higher score wins; equal scores are a draw.
fn play(tournament: &mut Tournament, first: u32, second: u32, score: [u32; 2]) -> Vec<Event> {
    let m = tournament
        .matches()
        .iter()
        .find(|m| {
            let seeds = m.slots().clone().map(|slot| slot.source().clone());
            seeds == [Source::Seed(first), Source::Seed(second)]
                || seeds == [Source::Seed(second), Source::Seed(first)]
        })
        .unwrap();
    let score = if m.slots()[0].source() == &Source::Seed(first) {
        score
    } else {
        [score[1], score[0]]
    };
    let winner = match score[0].cmp(&score[1]) {
        std::cmp::Ordering::Greater => Some(Side::A),
        std::cmp::Ordering::Less => Some(Side::B),
        std::cmp::Ordering::Equal => None,
    };
    let command = Command::ReportResult {
        match_id: m.id(),
        result: MatchResult {
            winner,
            score: Some(score),
        },
    };
    tournament.execute(command).unwrap()
}

/// The seeds in rank order.
fn ranking(tournament: &Tournament) -> Vec<u64> {
    tournament
        .standings()
        .unwrap()
        .iter()
        .map(|standing| standing.entrant.0 - 100)
        .collect()
}

#[test]
fn elimination_has_no_standings() {
    assert_eq!(create(Format::SingleElimination, 4).standings(), None);
    assert_eq!(create(Format::DoubleElimination, 4).standings(), None);
}

#[test]
fn standings_before_any_result_follow_seeds() {
    let tournament = round_robin(3);
    let standings = tournament.standings().unwrap();
    let expected: Vec<Standing> = (1..=3)
        .map(|seed| Standing {
            entrant: EntrantId(100 + seed),
            rank: seed as u32,
            played: 0,
            wins: 0,
            draws: 0,
            losses: 0,
            points: 0,
            score_for: 0,
            score_against: 0,
        })
        .collect();
    assert_eq!(standings, expected);
}

#[test]
fn points_decide() {
    let mut tournament = round_robin(2);
    play(&mut tournament, 2, 1, [1, 0]);
    let standings = tournament.standings().unwrap();
    assert_eq!(
        standings[0],
        Standing {
            entrant: EntrantId(102),
            rank: 1,
            played: 1,
            wins: 1,
            draws: 0,
            losses: 0,
            points: 3,
            score_for: 1,
            score_against: 0,
        }
    );
    assert_eq!(
        standings[1],
        Standing {
            entrant: EntrantId(101),
            rank: 2,
            played: 1,
            wins: 0,
            draws: 0,
            losses: 1,
            points: 0,
            score_for: 0,
            score_against: 1,
        }
    );
}

#[test]
fn head_to_head_decides() {
    let mut tournament = round_robin(4);
    play(&mut tournament, 1, 3, [10, 0]);
    play(&mut tournament, 1, 4, [10, 0]);
    play(&mut tournament, 2, 1, [1, 0]);
    play(&mut tournament, 3, 2, [1, 0]);
    play(&mut tournament, 2, 4, [1, 0]);
    play(&mut tournament, 4, 3, [1, 0]);
    // Seeds 1 and 2 have 6 points, and seed 2 beat seed 1 despite a worse
    // score difference. Seeds 3 and 4 have 3 points, and seed 4 beat seed 3.
    assert_eq!(ranking(&tournament), [2, 1, 4, 3]);
}

#[test]
fn score_difference_decides() {
    let mut tournament = round_robin(3);
    play(&mut tournament, 1, 2, [0, 0]);
    play(&mut tournament, 1, 3, [1, 0]);
    play(&mut tournament, 2, 3, [5, 0]);
    assert_eq!(ranking(&tournament), [2, 1, 3]);
}

#[test]
fn score_for_decides() {
    let mut tournament = round_robin(3);
    play(&mut tournament, 1, 2, [0, 0]);
    play(&mut tournament, 1, 3, [1, 0]);
    play(&mut tournament, 2, 3, [2, 1]);
    assert_eq!(ranking(&tournament), [2, 1, 3]);
}

#[test]
fn wins_decide() {
    let points = Points {
        win: 2,
        draw: 1,
        loss: 0,
    };
    let mut tournament = create(Format::RoundRobin { points }, 4);
    play(&mut tournament, 1, 2, [0, 0]);
    play(&mut tournament, 1, 3, [1, 1]);
    play(&mut tournament, 1, 4, [0, 0]);
    play(&mut tournament, 2, 3, [1, 0]);
    play(&mut tournament, 4, 2, [1, 0]);
    play(&mut tournament, 4, 3, [1, 0]);
    // Seeds 1 and 2 have 3 points, drew each other, and scored 1 for and 1
    // against. Seed 1 drew three times, and seed 2 won once.
    assert_eq!(ranking(&tournament), [4, 2, 1, 3]);
}

#[test]
fn seed_decides() {
    let mut tournament = round_robin(2);
    let events = play(&mut tournament, 2, 1, [1, 1]);
    assert_eq!(ranking(&tournament), [1, 2]);
    assert_eq!(
        events.last(),
        Some(&Event::Completed {
            champion: EntrantId(101)
        })
    );
    assert_eq!(
        tournament.status(),
        Status::Complete {
            champion: EntrantId(101)
        }
    );
}

#[test]
fn result_without_score_counts_as_zero() {
    let mut tournament = round_robin(2);
    let id = tournament.matches()[0].id();
    tournament
        .execute(Command::ReportResult {
            match_id: id,
            result: MatchResult {
                winner: Some(Side::B),
                score: None,
            },
        })
        .unwrap();
    let standings = tournament.standings().unwrap();
    assert_eq!(standings[0].entrant, EntrantId(102));
    assert!(
        standings
            .iter()
            .all(|s| s.score_for == 0 && s.score_against == 0)
    );
}

#[test]
fn totals_saturate() {
    let points = Points {
        win: u32::MAX,
        draw: 0,
        loss: 0,
    };
    let mut tournament = create(Format::RoundRobin { points }, 3);
    play(&mut tournament, 1, 2, [u32::MAX, 0]);
    play(&mut tournament, 1, 3, [u32::MAX, 0]);
    play(&mut tournament, 2, 3, [1, 0]);
    let standings = tournament.standings().unwrap();
    assert_eq!(standings[0].entrant, EntrantId(101));
    assert_eq!(standings[0].points, u32::MAX);
    assert_eq!(standings[0].score_for, u32::MAX);
    assert_eq!(standings[1].points, u32::MAX);
}
