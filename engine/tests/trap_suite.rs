//! The trap suite: real positions we walked into, judged by the rules alone.
//!
//! A paired run of fifty seeds resolves a difference of about a third of a
//! place and no less, so three iterations in a row came back "neutral" when all
//! that had been shown was that the instrument could not see them. The way out
//! is not more games but more events per game: one melee we lose by enclosure
//! leaves behind a dozen positions, and each one is an independent verdict.
//!
//! A position is labelled by an oracle that knows nothing but the rules. It is
//! the ordinary melee search with every positional weight set to zero, so the
//! only thing a leaf can say is "still alive"; a heading then scores at or
//! above zero exactly when it is not a forced death within the oracle's depth.
//! Nothing in the label comes from the valuation under test, so a change to the
//! valuation cannot flatter itself here.
//!
//! The heavy work is `#[ignore]`d and driven by `scripts/run-trapsuite`; the
//! tests that always run pin what the labels mean.

mod support;

use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};
use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard};
use tiger_engine::arena::serpent::Serpent;
use tiger_engine::lookahead::allowance::NeverStop;
use tiger_engine::lookahead::paranoid::MeleeSearcher;
use tiger_engine::rules_core::{SnakeState, TurnRequestDto, to_turn_state};
use tiger_engine::valuation::Assessor;
use tiger_engine::valuation::melee::enclosure::MeleeEnclosure;
use tiger_engine::valuation::melee::finish::MeleeFinish;
use tiger_engine::valuation::melee::weights::{DEFAULT_MELEE_PROFILE, MeleeWeights};
use tiger_engine::valuation::melee::{MeleeValuation, Surveyed};
use tiger_engine::valuation::weights::{DEFAULT_PROFILE, WeightSheet};

use support::{env_number, turn_state_from_bodies};

/// How deep the oracle looks unless `TRAP_SUITE_DEPTH` says otherwise. Our own
/// diagnosis put the trap three to five plies beyond a production search that
/// reaches four, so the oracle has to see about twice as far to be worth asking.
const ORACLE_DEPTH: u16 = 8;

// ---------------------------------------------------------------------------
// The oracle
// ---------------------------------------------------------------------------

/// The melee profile with every positional term at zero: the terminal scores
/// are untouched, so a leaf where we are alive is worth exactly nothing and the
/// only thing the search can still prefer is not being dead.
fn survival_melee_profile() -> MeleeWeights {
    MeleeWeights {
        territory_cell: 0,
        standing_segment: 0,
        hunger_urgency: 0,
        appetite_step: 0,
        larder_pellet: 0,
        head_danger: 0,
        attrition_seat: 0,
        finisher_step: 0,
        ..DEFAULT_MELEE_PROFILE
    }
}

/// The same for the duel terms, which the melee valuation falls back to once
/// only one opponent is left.
fn survival_duel_profile() -> WeightSheet {
    WeightSheet {
        territory_cell: 0,
        hunger_urgency: 0,
        length_advantage: 0,
        head_pressure: 0,
        enclosure_turn: 0,
        ..DEFAULT_PROFILE
    }
}

/// Whether each heading in [`Heading::ALL`] keeps us alive for `depth` plies
/// against the opponent model, judged with no valuation at all.
///
/// The trade risk is switched off: it is a deliberate bias of the production
/// root against a coin flip the platform sometimes takes, and it has no place
/// in a verdict about what the rules allow.
fn survival_of_each_heading(board: &MeleeBoard, depth: u16) -> [bool; 4] {
    let valuation =
        MeleeValuation::with_profiles(&survival_melee_profile(), &survival_duel_profile());
    let finish = MeleeFinish::new(&survival_melee_profile());
    Heading::ALL.map(|heading| {
        let mut searcher = MeleeSearcher::new(&valuation, finish).with_trade_risk(0);
        searcher
            .root_value(board, heading, depth, &mut NeverStop)
            .is_some_and(|value| value >= 0)
    })
}

/// Whether any heading survives: a position nobody could have saved is no
/// evidence about an engine and stays out of the suite.
fn is_savable(survival: &[bool; 4]) -> bool {
    survival.iter().any(|alive| *alive)
}

// ---------------------------------------------------------------------------
// What the labels mean (these always run)
// ---------------------------------------------------------------------------

/// A pocket of three cells our own body walls off, entered from its mouth, with
/// the open board one step the other way.
///
/// ```text
///   y=4  . . . . .        `U` is our head at (0,3) and `u` the rest of us;
///   y=3  U u . . .        the pocket `p` is (0,2), (0,1), (0,0), shut by our
///   y=2  p u . . .        own column at x=1 and by the left and bottom edges.
///   y=1  p u . . .        Going south costs nothing for three plies and
///   y=0  p u u u u        everything on the fourth.
/// ```
///
/// The wall is our body rather than a rival's on purpose: a rival beside the
/// mouth would answer the first step with a head-to-head, and the position
/// would be lost at once instead of four plies later, which is not the shape
/// that kills us in real games.
fn pocket_position() -> MeleeBoard {
    let ours: &[(i32, i32)] = &[
        (0, 3),
        (1, 3),
        (1, 2),
        (1, 1),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 0),
        (5, 0),
    ];
    let rival: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let third: &[(i32, i32)] = &[(9, 1), (9, 2), (9, 3)];
    let state = turn_state_from_bodies(&[ours, rival, third], &[90, 90, 90], 0, &[]);
    ingest_melee(&state).expect("three snakes make a melee")
}

#[test]
fn a_heading_that_kills_us_at_once_is_fatal_at_every_depth() {
    let board = pocket_position();
    // West leaves the board and East walks into our own second segment, which
    // is five turns from freeing.
    for depth in [1, 2, 5] {
        let survival = survival_of_each_heading(&board, depth);
        assert!(
            !survival[Heading::West.index()],
            "stepping off the board survives nothing, not even {depth} plies"
        );
        assert!(
            !survival[Heading::East.index()],
            "stepping into our own body survives nothing, not even {depth} plies"
        );
    }
}

#[test]
fn the_pocket_looks_safe_until_the_oracle_is_deep_enough() {
    // This is the whole point of the suite. Entering the pocket costs us
    // nothing for three plies and everything on the fourth: a search that stops
    // short calls it safe, and that is how we die.
    let board = pocket_position();

    let shallow = survival_of_each_heading(&board, 2);
    assert!(
        shallow[Heading::South.index()],
        "two plies cannot yet see the far end of the pocket"
    );

    let deep = survival_of_each_heading(&board, 5);
    assert!(
        !deep[Heading::South.index()],
        "five plies reach the far end of the pocket, where nothing is left"
    );
    assert!(deep[Heading::North.index()], "the open board is still open");
}

#[test]
fn a_position_with_one_way_out_is_savable_and_one_with_none_is_not() {
    let board = pocket_position();
    assert!(
        is_savable(&survival_of_each_heading(&board, 5)),
        "the open board is a way out, so the position counts as evidence"
    );

    // The same pocket three turns later: we are at the closed end of it, the
    // way back is our own neck and the wall is our own flank.
    let ours: &[(i32, i32)] = &[
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
    let state = turn_state_from_bodies(&[ours, rival, third], &[90, 90, 90], 0, &[]);
    let shut = ingest_melee(&state).expect("three snakes make a melee");

    assert!(
        !is_savable(&survival_of_each_heading(&shut, 5)),
        "a position already lost says nothing about the engine that reached it"
    );
}

#[test]
fn the_oracle_reads_survival_and_nothing_else() {
    // North gives up the whole board and South keeps it, but both are alive in
    // five plies, so the oracle must call both safe. A label that preferred the
    // roomier heading would be the valuation judging itself.
    let ours: &[(i32, i32)] = &[(5, 1), (5, 0), (4, 0)];
    let rival: &[(i32, i32)] = &[(7, 7), (7, 8), (7, 9)];
    let third: &[(i32, i32)] = &[(0, 9), (0, 8), (0, 7)];
    let state = turn_state_from_bodies(&[ours, rival, third], &[90, 90, 90], 0, &[]);
    let board = ingest_melee(&state).expect("three snakes make a melee");

    let survival = survival_of_each_heading(&board, 5);
    assert!(
        survival[Heading::North.index()] && survival[Heading::East.index()],
        "both are alive in five plies, whatever they are worth positionally"
    );
}

// ---------------------------------------------------------------------------
// The suite proper (driven by scripts/run-trapsuite)
// ---------------------------------------------------------------------------

/// One harvested position: where it came from, the `/move` request body that
/// reproduces it, and what the engine of the day actually played.
struct Harvested {
    seed: u64,
    turn: u64,
    turns_before_death: u64,
    played: Heading,
    request: Value,
}

fn heading_from_name(name: &str) -> Option<Heading> {
    match name {
        "up" => Some(Heading::North),
        "right" => Some(Heading::East),
        "down" => Some(Heading::South),
        "left" => Some(Heading::West),
        _ => None,
    }
}

fn heading_name(heading: Heading) -> &'static str {
    match heading {
        Heading::North => "up",
        Heading::East => "right",
        Heading::South => "down",
        Heading::West => "left",
    }
}

/// The file `name` names under `target/trap-suite/`, or whatever `variable`
/// says. A test runs from the crate directory, not the workspace root, so the
/// default is anchored to the manifest rather than to the current directory.
fn suite_path(variable: &str, name: &str) -> PathBuf {
    std::env::var(variable).map_or_else(
        |_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../target/trap-suite")
                .join(name)
        },
        PathBuf::from,
    )
}

fn read_harvest(path: &PathBuf) -> Vec<Harvested> {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read the harvest at {}: {error}", path.display()));
    let value: Value = serde_json::from_str(&text).expect("the harvest must be JSON");
    value["positions"]
        .as_array()
        .expect("the harvest must hold an array of positions")
        .iter()
        .map(|entry| Harvested {
            seed: entry["seed"].as_u64().expect("a seed"),
            turn: entry["turn"].as_u64().expect("a turn"),
            turns_before_death: entry["turns_before_death"].as_u64().expect("a distance"),
            played: heading_from_name(entry["played"].as_str().expect("a move name"))
                .expect("a known move name"),
            request: entry["request"].clone(),
        })
        .collect()
}

/// The position of a `/move` request as a board the oracle can search.
///
/// This does not go through `ingest_melee`, which insists on three or four
/// snakes because that is when the server takes the melee path. The oracle asks
/// only what the rules allow, and the rules do not change when the third snake
/// dies: a great many of these games are already down to a duel by the time we
/// walk into the pocket, and dropping them would throw away the evidence.
fn board_of(request: &Value) -> Option<MeleeBoard> {
    let dto: TurnRequestDto = serde_json::from_value(request.clone()).ok()?;
    let state = to_turn_state(&dto).ok()?;
    let snakes = state.snakes();
    if !(2..=MAX_SEATS).contains(&snakes.len()) {
        return None;
    }
    let mut serpents = vec![to_serpent(state.you())?];
    for (index, snake) in snakes.iter().enumerate() {
        if index != state.you_index() {
            serpents.push(to_serpent(snake)?);
        }
    }
    MeleeBoard::try_new(&serpents, CellSet::from_bits(state.food().bits())).ok()
}

/// One snake of a request as a serpent, head first, exactly as `ingest` does it.
fn to_serpent(snake: &SnakeState) -> Option<Serpent> {
    let body = snake
        .body()
        .iter()
        .map(|cell| Cell::from_index(cell.value()))
        .collect::<Option<Vec<_>>>()?;
    Serpent::new(&body, snake.health()).ok()
}

/// Labels every harvested position and writes the suite: which headings survive
/// the oracle's depth, whether the position was savable at all, and whether the
/// move actually played was one of the fatal ones.
#[test]
#[ignore = "driven by scripts/run-trapsuite; minutes of work"]
fn label_the_trap_suite() {
    let harvest = suite_path("TRAP_SUITE_HARVEST", "positions.json");
    let out = suite_path("TRAP_SUITE_LABELS", "labels.json");
    let depth = u16::try_from(env_number::<u64>(
        "TRAP_SUITE_DEPTH",
        u64::from(ORACLE_DEPTH),
    ))
    .expect("a depth fits in u16");

    let positions = read_harvest(&harvest);
    let mut labelled = Vec::new();
    let (mut savable, mut traps) = (0usize, 0usize);
    let mut traps_by_seats = [0usize; MAX_SEATS + 1];
    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let seats = board.alive_count() as usize;
        let survival = survival_of_each_heading(&board, depth);
        let alive: Vec<&str> = Heading::ALL
            .iter()
            .filter(|heading| survival[heading.index()])
            .map(|heading| heading_name(*heading))
            .collect();
        let is_trap = is_savable(&survival) && !survival[position.played.index()];
        savable += usize::from(is_savable(&survival));
        traps += usize::from(is_trap);
        traps_by_seats[seats] += usize::from(is_trap);
        labelled.push(json!({
            "seed": position.seed,
            "turn": position.turn,
            "turns_before_death": position.turns_before_death,
            "seats": seats,
            "played": heading_name(position.played),
            "surviving": alive,
            "savable": is_savable(&survival),
            "trap": is_trap,
            "request": position.request,
        }));
    }

    println!("trap suite: {} positions", positions.len());
    println!("  savable at depth {depth}: {savable}");
    println!("  of those, walked into a trap: {traps}");
    for (seats, count) in traps_by_seats.iter().enumerate().skip(2) {
        println!("    with {seats} snakes alive: {count}");
    }

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).expect("the output directory must be creatable");
    }
    fs::write(
        &out,
        serde_json::to_string_pretty(&json!({
            "oracle_depth": depth,
            "positions": labelled,
        }))
        .expect("the labels must serialize"),
    )
    .expect("the labels must be writable");
    println!("  written to {}", out.display());
}

/// Says how often a term actually speaks on the positions that matter.
///
/// The parity bound of iteration 17 was correct, tested and inert: over 93 real
/// positions the walkable area equalled the cell count, so the term could not
/// have changed a decision. A term is worth a weight only once it has been
/// shown to fire where we lose, and this is the cheapest way to show it.
#[test]
#[ignore = "driven by scripts/run-trapsuite; needs a harvest"]
fn measure_the_enclosure_term_on_the_harvest() {
    let harvest = suite_path("TRAP_SUITE_HARVEST", "positions.json");
    let positions = read_harvest(&harvest);

    let (mut read, mut spoke) = (0usize, 0usize);
    let (mut against_us, mut worst) = (0usize, 0i32);
    let mut by_distance: Vec<(u64, i32)> = Vec::new();
    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        read += 1;
        let value = MeleeEnclosure.assess(&Surveyed::new(&board));
        if value != 0 {
            spoke += 1;
            against_us += usize::from(value < 0);
            worst = worst.min(value);
            by_distance.push((position.turns_before_death, value));
        }
    }

    by_distance.sort_unstable();
    println!("enclosure term over {read} harvested positions");
    println!("  not silent: {spoke}");
    println!("  and against us: {against_us} (worst {worst})");
    for (distance, value) in &by_distance {
        println!("    {distance} turns before we died: {value}");
    }
}
