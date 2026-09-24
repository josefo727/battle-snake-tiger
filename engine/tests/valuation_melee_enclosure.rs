mod support;

use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::{MeleeBoard, Seat};
use tiger_engine::valuation::Assessor;
use tiger_engine::valuation::melee::Surveyed;
use tiger_engine::valuation::melee::enclosure::MeleeEnclosure;

use support::turn_state_from_bodies;

fn melee(bodies: &[&[(i32, i32)]], health: &[i32], food: &[(i32, i32)]) -> MeleeBoard {
    let state = turn_state_from_bodies(bodies, health, 0, food);
    ingest_melee(&state).expect("a melee converts")
}

/// Our head in a pocket a rival's body walls off, with both rivals outside it.
///
/// ```text
///   y=4  W w . . .        `U` is our head at (0,1) and `u` the rest of us;
///   y=3  u w . . .        `W`/`w` is the rival that shuts the pocket, its
///   y=2  u w . . .        head at (0,4) and its body down the column at x=1
///   y=1  U w . . .        and away along y=0. The only ground we have left
///   y=0  . w w w .        is the cell below us and what our own tail frees.
/// ```
///
/// The wall has to be a rival's: a pocket shut by our own body is no pocket at
/// all, because our tail gives the way out back one cell at a time.
fn sealed_pocket(our_health: i32) -> MeleeBoard {
    let us: &[(i32, i32)] = &[(0, 1), (0, 2), (0, 3)];
    let wall: &[(i32, i32)] = &[
        (0, 4),
        (1, 4),
        (1, 3),
        (1, 2),
        (1, 1),
        (1, 0),
        (2, 0),
        (3, 0),
    ];
    let far: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    melee(&[us, wall, far], &[our_health, 90, 90], &[])
}

#[test]
fn a_seat_shut_behind_a_rival_wall_is_worth_less_than_the_open_board() {
    let board = sealed_pocket(90);

    assert_eq!(
        MeleeEnclosure.survival_estimate(&board, Seat::US),
        3,
        "three cells of one colour and two of the other are three turns of path"
    );
    assert!(
        MeleeEnclosure.assess(&Surveyed::new(&board)) < 0,
        "the pocket is worth fewer turns than the board the rivals still have"
    );
}

#[test]
fn the_term_is_silent_while_anyone_can_still_reach_our_ground() {
    // Three serpents in the open, every one of them able to walk to the others:
    // nobody's survival is bounded by a wall yet, so the term says nothing and
    // leaves the position to the terms that read territory.
    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let rival: &[(i32, i32)] = &[(1, 1), (1, 2), (1, 3)];
    let third: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let board = melee(&[us, rival, third], &[90, 90, 90], &[]);

    assert_eq!(MeleeEnclosure.assess(&Surveyed::new(&board)), 0);
}

#[test]
fn a_pocket_our_own_body_shuts_is_no_pocket_at_all() {
    // The shape that fooled the first draft of this test: our head at the
    // closed end of a run of our own body. The tail gives the ground back one
    // cell at a time, so the room is the whole board and the term is silent.
    let us: &[(i32, i32)] = &[
        (0, 0),
        (0, 1),
        (0, 2),
        (0, 3),
        (1, 3),
        (1, 2),
        (1, 1),
        (1, 0),
        (2, 0),
    ];
    let rival: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let third: &[(i32, i32)] = &[(9, 1), (9, 2), (9, 3)];
    let board = melee(&[us, rival, third], &[90, 90, 90], &[]);

    assert!(
        MeleeEnclosure.survival_estimate(&board, Seat::US) > 50,
        "following our own tail leads back out to the open board"
    );
}

#[test]
fn hunger_caps_the_room_when_the_room_outlasts_the_health() {
    // On the open board the colours allow a long walk, so what bounds a
    // serpent is the health it has to walk it.
    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let rival: &[(i32, i32)] = &[(1, 1), (1, 2), (1, 3)];
    let third: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];

    let starving = melee(&[us, rival, third], &[7, 90, 90], &[]);
    assert_eq!(
        MeleeEnclosure.survival_estimate(&starving, Seat::US),
        7,
        "seven turns of health are seven turns of life, whatever the room"
    );

    let fed = melee(&[us, rival, third], &[90, 90, 90], &[]);
    assert!(
        MeleeEnclosure.survival_estimate(&fed, Seat::US) > 7,
        "with health to spare the room is what bounds us, not hunger"
    );
}

#[test]
fn the_pocket_is_worth_more_with_a_pellet_in_it() {
    // A pellet resets health, so a room with food in it is worth more turns
    // than the same room without: the estimate has to know that before it can
    // be compared with a rival's.
    let empty = MeleeEnclosure.survival_estimate(&sealed_pocket(2), Seat::US);

    let us: &[(i32, i32)] = &[(0, 1), (0, 2), (0, 3)];
    let wall: &[(i32, i32)] = &[
        (0, 4),
        (1, 4),
        (1, 3),
        (1, 2),
        (1, 1),
        (1, 0),
        (2, 0),
        (3, 0),
    ];
    let far: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let fed = melee(&[us, wall, far], &[2, 90, 90], &[(0, 0)]);

    assert_eq!(empty, 2, "two turns of health, and the room outlasts them");
    assert!(
        MeleeEnclosure.survival_estimate(&fed, Seat::US) > empty,
        "the pellet in the pocket buys turns the bare pocket does not"
    );
}

#[test]
fn a_dead_seat_is_worth_nothing() {
    // The assessor has to answer for a board with a seat already gone, and a
    // seat that is gone has no room at all.
    let us: &[(i32, i32)] = &[(0, 0), (0, 1), (0, 2)];
    let rival: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let third: &[(i32, i32)] = &[(5, 5), (5, 6), (5, 7)];
    let board = melee(&[us, rival, third], &[90, 90, 90], &[]);

    for seat in board.seats() {
        assert!(
            MeleeEnclosure.survival_estimate(&board, seat) > 0,
            "every living seat has somewhere to go on an empty board"
        );
    }
    assert_eq!(
        MeleeEnclosure.survival_estimate(&board, Seat::ALL[3]),
        0,
        "the fourth seat is not playing"
    );
}
