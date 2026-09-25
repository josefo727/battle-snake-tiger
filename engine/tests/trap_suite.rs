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
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome, Seat};
use tiger_engine::arena::serpent::Serpent;
use tiger_engine::gateway::clock::SystemClock;
use tiger_engine::lookahead::allowance::{NeverStop, SearchAllowance};
use tiger_engine::lookahead::deepening::{DEPTH_CEILING, deepen_melee};
use tiger_engine::lookahead::paranoid::{MeleeSearcher, TRADE_RISK, contested_by_equal};
use tiger_engine::lookahead::rollout;
use tiger_engine::rules_core::{Clock, RequestTiming, SnakeState, TurnRequestDto, to_turn_state};
use tiger_engine::valuation::Assessor;
use tiger_engine::valuation::melee::appetite::Appetite;
use tiger_engine::valuation::melee::enclosure::MeleeEnclosure;
use tiger_engine::valuation::melee::finish::MeleeFinish;
use tiger_engine::valuation::melee::hunger::Hunger;
use tiger_engine::valuation::melee::larder::Larder;
use tiger_engine::valuation::melee::territory::Territory;
use tiger_engine::valuation::melee::weights::{DEFAULT_MELEE_PROFILE, MeleeWeights};
use tiger_engine::valuation::melee::{MeleeValuation, Surveyed};
use tiger_engine::valuation::weights::{DEFAULT_PROFILE, WeightSheet};
use tiger_engine::verdict::service::VerdictService;

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

// ---------------------------------------------------------------------------
// A reference that can see further than a search can
// ---------------------------------------------------------------------------

/// How many turns a rollout plays before it gives up and calls us a survivor.
const ROLLOUT_HORIZON: u16 = 30;
/// How many rollouts back each heading.
const ROLLOUTS: u32 = 200;
/// The fixed seed: the reference has to be the same number every time it is
/// asked, or two engines cannot be compared on it.
const ROLLOUT_SEED: u64 = 0x5f2d_1c9b_a733_4e11;

/// The oracle proved the mistake that kills us is not inside an eight-ply
/// horizon, and a search cannot be pushed to thirty: every ply costs about five
/// times the last, so depth 12 is hours for a single position.
///
/// What can reach thirty turns is simulation. From each heading we play many
/// games in which every serpent, ourselves included, steps at random among the
/// steps that do not kill it outright, and we count how long we last. A random
/// walker dies in a corridor a careful serpent would survive, so this reads
/// open ground as worth more than it is; that bias is the same for every
/// heading of a position and for every engine measured on it, and it is the
/// only reference available that sees as far as the mistake.
///
/// No food is spawned: the board's own pellets are eaten as usual, but nothing
/// new appears, so the number depends on the position and the seed alone.
fn rollout_survival(board: &MeleeBoard, heading: Heading, seed: u64) -> f64 {
    let mut rng = Xorshift::new(seed);
    let mut total = 0u32;
    for _ in 0..ROLLOUTS {
        total += u32::from(one_rollout(board, heading, &mut rng));
    }
    f64::from(total) / f64::from(ROLLOUTS)
}

/// Turns we last from `board` after playing `heading`, capped at the horizon.
fn one_rollout(board: &MeleeBoard, heading: Heading, rng: &mut Xorshift) -> u16 {
    let mut current = *board;
    let mut first = Some(heading);
    for turn in 0..ROLLOUT_HORIZON {
        let mut moves = [Heading::North; MAX_SEATS];
        for seat in current.seats() {
            moves[seat.index()] = if seat == Seat::US && first.is_some() {
                first.take().expect("checked")
            } else {
                safe_random_step(&current, seat, rng)
            };
        }
        match current.advance(&moves) {
            MeleeOutcome::Continues(next) => current = next,
            MeleeOutcome::WeAlone => return ROLLOUT_HORIZON,
            MeleeOutcome::WeDown { .. } => return turn,
        }
    }
    ROLLOUT_HORIZON
}

/// A step for `seat` drawn evenly from the ones that do not kill it on the
/// spot; when every step does, any of them, because it makes no difference.
fn safe_random_step(board: &MeleeBoard, seat: Seat, rng: &mut Xorshift) -> Heading {
    let head = board.serpent(seat).head();
    let enterable = board
        .seats()
        .fold(board.occupied().complement(), |cells, other| {
            board
                .serpent(other)
                .cell_released_on_turn(1)
                .map_or(cells, |cell| cells.with(cell))
        });
    let safe: Vec<Heading> = Heading::ALL
        .into_iter()
        .filter(|h| h.step(head).is_some_and(|cell| enterable.contains(cell)))
        .collect();
    if safe.is_empty() {
        return Heading::ALL[(rng.next() % 4) as usize];
    }
    safe[(rng.next() % safe.len() as u64) as usize]
}

/// The smallest reproducible source of randomness that will do the job; a
/// dependency would have to be justified to cargo-deny for four lines of shift.
struct Xorshift(u64);

impl Xorshift {
    const fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn the_rollout_reference_sees_the_pocket_the_search_needs_five_plies_for() {
    // The same pocket as above. The reference is asked nothing about plies: it
    // just plays on, and a serpent that walks into three cells is dead in four
    // turns however the rest of the game would have gone.
    let board = pocket_position();

    let into_the_pocket = rollout_survival(&board, Heading::South, ROLLOUT_SEED);
    let into_the_open = rollout_survival(&board, Heading::North, ROLLOUT_SEED);

    assert!(
        into_the_pocket < 5.0,
        "the pocket is four turns long, whatever happens afterwards (got {into_the_pocket})"
    );
    assert!(
        into_the_open > into_the_pocket + 5.0,
        "the open board is worth many more turns (got {into_the_open})"
    );
}

#[test]
fn the_rollout_reference_gives_the_same_answer_every_time() {
    // Two engines can only be compared on this number if it is the same number
    // both times it is asked.
    let board = pocket_position();
    assert_eq!(
        rollout_survival(&board, Heading::North, ROLLOUT_SEED),
        rollout_survival(&board, Heading::North, ROLLOUT_SEED)
    );
}

/// What the engine as compiled answers for `board`, so the suite scores the
/// engine and not a weight sheet.
///
/// The depth is fixed rather than timed. A clock would make the score depend on
/// what else the machine was doing, and an instrument built to settle arguments
/// between two engines cannot answer differently because a sparring run happened
/// to be going at the time. Four is the median depth the melee profiling harness
/// reaches under the production allowance; `TRAP_SUITE_PLAY_DEPTH` moves it.
fn production_choice(board: &MeleeBoard) -> Option<Heading> {
    choice_with(board, &MeleeValuation::standard())
}

/// The same with a valuation of our choosing, so a weight can be swept without
/// rebuilding the engine once per value.
fn choice_with(board: &MeleeBoard, valuation: &MeleeValuation) -> Option<Heading> {
    choice_at_risk(board, valuation, TRADE_RISK)
}

/// The same with the root's trade risk set by hand, for sweeping that dial.
fn choice_at_risk(board: &MeleeBoard, valuation: &MeleeValuation, risk: i32) -> Option<Heading> {
    let finish = MeleeFinish::new(&DEFAULT_MELEE_PROFILE);
    let depth =
        u16::try_from(env_number::<u64>("TRAP_SUITE_PLAY_DEPTH", 4)).expect("a depth fits in u16");
    let mut searcher = MeleeSearcher::new(valuation, finish).with_trade_risk(risk);
    searcher.search_fixed(board, depth).best
}

/// Scores the engine as compiled against the rollout reference.
///
/// A position counts only when the headings differ by at least
/// `TRAP_SUITE_MARGIN` turns of expected life: where every step is worth the
/// same there is nothing to get right. On the rest the engine is asked for its
/// move at a fixed depth, and the report is how much of the available life it
/// kept.
#[test]
#[ignore = "driven by scripts/run-trapsuite; minutes of work"]
fn score_the_trap_suite() {
    let harvest = suite_path("TRAP_SUITE_HARVEST", "positions.json");
    let margin: f64 = env_number("TRAP_SUITE_MARGIN", 3.0);
    let positions = read_harvest(&harvest);

    let (mut informative, mut took_the_best) = (0usize, 0usize);
    let (mut kept, mut available) = (0.0f64, 0.0f64);
    let mut played_kept = 0.0f64;
    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let reference = Heading::ALL.map(|h| rollout_survival(&board, h, ROLLOUT_SEED));
        let best = reference.iter().copied().fold(f64::MIN, f64::max);
        let worst = reference.iter().copied().fold(f64::MAX, f64::min);
        if best - worst < margin {
            continue;
        }
        informative += 1;
        available += best;
        played_kept += reference[position.played.index()];
        let Some(chosen) = production_choice(&board) else {
            continue;
        };
        kept += reference[chosen.index()];
        took_the_best += usize::from(reference[chosen.index()] >= best - 1.0);
    }

    println!("trap suite scored over {} positions", positions.len());
    println!("  positions where the step matters (margin {margin}): {informative}");
    if informative > 0 {
        println!(
            "  life kept by this engine: {:.1}% of what was there",
            100.0 * kept / available
        );
        println!(
            "  life kept by the move actually played then: {:.1}%",
            100.0 * played_kept / available
        );
        println!(
            "  took a best step (within one turn): {took_the_best} of {informative} ({:.1}%)",
            100.0 * took_the_best as f64 / informative as f64
        );
    }
}

/// Says what the food terms can see on ordinary play.
///
/// Appetite, Larder and Hunger all read `survey.food[US]` or
/// `survey.owned[US]`: the pellets we reach strictly before every rival. In a
/// melee that means beating three of them, and a pellet two equal-length
/// serpents reach on the same turn belongs to nobody. This counts how often
/// that leaves the three terms with nothing to say.
#[test]
#[ignore = "driven by scripts/run-trapsuite; needs a cross-section sample"]
fn measure_what_the_food_terms_can_see() {
    let harvest = suite_path("TRAP_SUITE_SAMPLE", "sample.json");
    let positions = read_harvest(&harvest);

    let (mut read, mut we_own_none, mut no_pellets_at_all) = (0usize, 0usize, 0usize);
    let (mut all_three_silent, mut hunger_spoke) = (0usize, 0usize);
    let (mut ours_total, mut best_rival_total, mut nobody_total, mut on_board_total) =
        (0u32, 0u32, 0u32, 0u32);
    let mut rival_owns_none = 0usize;
    let (mut rival_seats, mut rival_owns_none_any, mut rivals_total) = (0usize, 0usize, 0u32);
    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let pellets = board.pellets();
        if pellets.is_empty() {
            no_pellets_at_all += 1;
            continue;
        }
        read += 1;
        let survey = Territory.survey(&board);
        let owned_by = |seat: Seat| survey.owned[seat.index()].intersection(pellets).len();
        let ours = owned_by(Seat::US);
        let rivals: Vec<u32> = board
            .seats()
            .filter(|seat| *seat != Seat::US)
            .map(owned_by)
            .collect();
        let best_rival = rivals.iter().copied().max().unwrap_or(0);
        rival_seats += rivals.len();
        rival_owns_none_any += rivals.iter().filter(|owned| **owned == 0).count();
        rivals_total += rivals.iter().sum::<u32>();
        let claimed: u32 = ours + rivals.iter().sum::<u32>();
        on_board_total += pellets.len();
        ours_total += ours;
        best_rival_total += best_rival;
        nobody_total += pellets.len() - claimed;
        let surveyed = Surveyed::new(&board);
        let appetite = Appetite.assess(&surveyed);
        let larder = Larder.assess(&surveyed);
        let hunger = Hunger.assess(&surveyed);
        all_three_silent += usize::from(appetite == 0 && larder == 0 && hunger == 0);
        hunger_spoke += usize::from(hunger != 0);
        we_own_none += usize::from(ours == 0);
        rival_owns_none += usize::from(best_rival == 0);
    }

    println!("food the survey grants, over {read} sampled positions");
    println!("  (skipped {no_pellets_at_all} with no pellet on the board)");
    println!(
        "  positions where we own no pellet at all: {we_own_none} ({:.0}%)",
        100.0 * we_own_none as f64 / read as f64
    );
    println!(
        "  positions where the best rival owns none: {rival_owns_none} ({:.0}%)",
        100.0 * rival_owns_none as f64 / read as f64
    );
    println!("  pellets on the board: {on_board_total}");
    println!(
        "  of them ours {ours_total}, the best rival's {best_rival_total}, nobody's {nobody_total} ({:.0}% unclaimed)",
        100.0 * f64::from(nobody_total) / f64::from(on_board_total)
    );
    println!(
        "  per rival seat, owns no pellet in {rival_owns_none_any} of {rival_seats} ({:.0}%); pellets per rival seat {:.2} against our {:.2}",
        100.0 * rival_owns_none_any as f64 / rival_seats as f64,
        f64::from(rivals_total) / rival_seats as f64,
        f64::from(ours_total) / read as f64
    );
    println!(
        "  positions where appetite, larder and hunger are all exactly zero: {all_three_silent} ({:.0}%)",
        100.0 * all_three_silent as f64 / read as f64
    );
    println!(
        "  positions where hunger says anything at all: {hunger_spoke} ({:.0}%)",
        100.0 * hunger_spoke as f64 / read as f64
    );
}

/// Says which term decides the step, rather than which term has the largest
/// weight.
///
/// For every sampled position the four one-ply children are scored with the
/// full ledger (the rivals holding a fixed self-preserving heading, the same one
/// in every child, so the only thing that differs between children is our own
/// step). The spread of a term across those children is what that term is worth
/// to the decision; a term with a huge weight and no spread decides nothing.
#[test]
#[ignore = "driven by scripts/run-trapsuite; needs a cross-section sample"]
fn measure_which_term_decides_the_step() {
    let harvest = suite_path("TRAP_SUITE_SAMPLE", "sample.json");
    let positions = read_harvest(&harvest);
    let valuation = MeleeValuation::standard();

    let mut spread: Vec<(String, Vec<i32>)> = Vec::new();
    let mut counted = 0usize;
    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let mut rivals = [Heading::North; MAX_SEATS];
        for seat in board.seats().filter(|seat| *seat != Seat::US) {
            rivals[seat.index()] = first_self_preserving(&board, seat);
        }

        let mut per_term: Vec<Vec<i32>> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        for ours in Heading::ALL {
            let mut moves = rivals;
            moves[Seat::US.index()] = ours;
            let MeleeOutcome::Continues(child) = board.advance(&moves) else {
                continue;
            };
            let Some(assessed) = valuation.assess(&child) else {
                continue;
            };
            for (index, entry) in assessed.ledger.entries().iter().enumerate() {
                if per_term.len() <= index {
                    per_term.push(Vec::new());
                    names.push(entry.name.to_owned());
                }
                per_term[index].push(entry.contribution());
            }
        }
        if per_term.first().is_none_or(|values| values.len() < 2) {
            continue;
        }
        counted += 1;
        if spread.is_empty() {
            spread = names
                .iter()
                .map(|name| (name.clone(), Vec::new()))
                .collect();
        }
        for (index, values) in per_term.iter().enumerate() {
            let high = values.iter().copied().max().unwrap_or(0);
            let low = values.iter().copied().min().unwrap_or(0);
            spread[index].1.push(high - low);
        }
    }

    println!("what each term is worth to the step, over {counted} sampled positions");
    println!("  (the spread of its contribution across our four one-ply children)");
    let mut rows: Vec<(String, f64, i32, usize)> = spread
        .iter()
        .map(|(name, values)| {
            let mean = values.iter().map(|v| f64::from(*v)).sum::<f64>() / values.len() as f64;
            let top = values.iter().copied().max().unwrap_or(0);
            let silent = values.iter().filter(|v| **v == 0).count();
            (name.clone(), mean, top, silent)
        })
        .collect();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!(
        "{:<18}{:>12}{:>12}{:>14}",
        "term", "mean", "largest", "silent"
    );
    for (name, mean, top, silent) in rows {
        println!(
            "{name:<18}{mean:>12.0}{top:>12}{:>13.0}%",
            100.0 * silent as f64 / counted as f64
        );
    }
}

/// The first heading in the fixed order that does not kill `seat` outright.
fn first_self_preserving(board: &MeleeBoard, seat: Seat) -> Heading {
    let head = board.serpent(seat).head();
    let enterable = board
        .seats()
        .fold(board.occupied().complement(), |cells, other| {
            board
                .serpent(other)
                .cell_released_on_turn(1)
                .map_or(cells, |cell| cells.with(cell))
        });
    Heading::ALL
        .into_iter()
        .find(|h| h.step(head).is_some_and(|cell| enterable.contains(cell)))
        .unwrap_or(Heading::ALL[0])
}

/// Sweeps the weights that are supposed to make us grow, along the two axes
/// they trade between.
///
/// The food axis is not how near the chosen step leaves us to a pellet -- that
/// was the axis the first attempt optimised, and it rewarded sitting beside
/// food rather than taking it. It is now the plain question: of the positions
/// where a step onto a pellet was there to be taken, how many did we take?
#[test]
#[ignore = "driven by scripts/run-trapsuite; needs a harvest and a sample"]
fn sweep_the_growth_weights() {
    let traps = read_harvest(&suite_path("TRAP_SUITE_HARVEST", "positions.json"));
    let sample = read_harvest(&suite_path("TRAP_SUITE_SAMPLE", "sample.json"));
    let grid: Vec<(i32, i32)> = std::env::var("TRAP_SUITE_GROWTH_SWEEP")
        .unwrap_or_else(|_| "0:1000,0:2000,0:3000,0:4000,0:6000".to_owned())
        .split(',')
        .filter_map(|pair| {
            let (craving, standing) = pair.trim().split_once(':')?;
            Some((craving.parse().ok()?, standing.parse().ok()?))
        })
        .collect();

    // The reference is the same for every cell of the sweep, so it is computed
    // once; recomputing it per weight was most of the cost of the last one.
    let mut references: Vec<(MeleeBoard, [f64; 4], f64)> = Vec::new();
    for position in &traps {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let reference = Heading::ALL.map(|h| rollout_survival(&board, h, ROLLOUT_SEED));
        let top = reference.iter().copied().fold(f64::MIN, f64::max);
        let low = reference.iter().copied().fold(f64::MAX, f64::min);
        if top - low >= 3.0 {
            references.push((board, reference, top));
        }
    }

    // Likewise the sampled positions in which a meal was on offer at all.
    let mut meals: Vec<(MeleeBoard, [bool; 4])> = Vec::new();
    for position in &sample {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let offered = meals_on_offer(&board);
        if offered.iter().any(|meal| *meal) {
            meals.push((board, offered));
        }
    }

    println!(
        "growth sweep over {} trap positions and {} sampled ones with a meal on offer",
        references.len(),
        meals.len()
    );
    println!(
        "{:<10}{:<12}{:>14}{:>16}{:>16}",
        "craving", "standing", "life kept", "best step", "meals taken"
    );
    for (craving, standing) in grid {
        let _ = craving;
        let profile = MeleeWeights {
            standing_segment: standing,
            ..DEFAULT_MELEE_PROFILE
        };
        let valuation = MeleeValuation::with_profiles(&profile, &DEFAULT_PROFILE);

        let (mut kept, mut available, mut best) = (0.0f64, 0.0f64, 0usize);
        for (board, reference, top) in &references {
            available += top;
            if let Some(chosen) = choice_with(board, &valuation) {
                kept += reference[chosen.index()];
                best += usize::from(reference[chosen.index()] >= top - 1.0);
            }
        }

        let mut taken = 0usize;
        for (board, offered) in &meals {
            if let Some(chosen) = choice_with(board, &valuation)
                && offered[chosen.index()]
            {
                taken += 1;
            }
        }

        println!(
            "{craving:<10}{standing:<12}{:>13.1}%{:>11} of {:<4}{:>11} of {:<4}",
            100.0 * kept / available,
            best,
            references.len(),
            taken,
            meals.len()
        );
    }
}

/// Which of our four headings would put our head on a pellet next turn, and is
/// not simply suicide: a meal we could not survive is not a meal refused.
fn meals_on_offer(board: &MeleeBoard) -> [bool; 4] {
    let head = board.serpent(Seat::US).head();
    let pellets = board.pellets();
    let mut rivals = [Heading::North; MAX_SEATS];
    for seat in board.seats().filter(|seat| *seat != Seat::US) {
        rivals[seat.index()] = first_self_preserving(board, seat);
    }
    Heading::ALL.map(|heading| {
        let Some(target) = heading.step(head) else {
            return false;
        };
        if !pellets.contains(target) {
            return false;
        }
        let mut moves = rivals;
        moves[Seat::US.index()] = heading;
        matches!(board.advance(&moves), MeleeOutcome::Continues(_))
    })
}

/// Explains one position heading by heading: what the production valuation is
/// worth, what it is worth with the trade risk switched off, whether an
/// equal-length rival can take the same cell, and what the rollouts say.
///
/// For reading a single decision from a real game, which a mean over a suite
/// cannot do.
#[test]
#[ignore = "driven by hand: TRAP_SUITE_HARVEST=<one-position file>"]
fn explain_the_positions() {
    let positions = read_harvest(&suite_path("TRAP_SUITE_HARVEST", "positions.json"));
    let depth =
        u16::try_from(env_number::<u64>("TRAP_SUITE_PLAY_DEPTH", 4)).expect("a depth fits in u16");
    let valuation = MeleeValuation::standard();
    let finish = MeleeFinish::new(&DEFAULT_MELEE_PROFILE);

    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        println!(
            "seed {} turn {} played {} at depth {depth}",
            position.seed,
            position.turn,
            heading_name(position.played)
        );
        println!(
            "{:<8}{:>14}{:>14}{:>10}{:>14}",
            "heading", "value", "no trade risk", "contested", "rollout life"
        );
        for heading in Heading::ALL {
            let mut with = MeleeSearcher::new(&valuation, finish);
            let mut without = MeleeSearcher::new(&valuation, finish).with_trade_risk(0);
            let target = heading.step(board.serpent(Seat::US).head());
            let contested = target.is_some_and(|cell| contested_by_equal(&board, cell));
            let a = with.root_value(&board, heading, depth, &mut NeverStop);
            let b = without.root_value(&board, heading, depth, &mut NeverStop);
            println!(
                "{:<8}{:>14}{:>14}{:>10}{:>14.2}",
                heading_name(heading),
                a.map_or("-".to_owned(), |v| v.to_string()),
                b.map_or("-".to_owned(), |v| v.to_string()),
                if contested { "yes" } else { "no" },
                rollout_survival(&board, heading, ROLLOUT_SEED)
            );
        }
        println!(
            "  chosen by this engine: {}",
            production_choice(&board).map_or("-".to_owned(), heading_name_owned)
        );
        println!("  what the weights that matter would have chosen:");
        for (label, profile) in [
            ("iteration 19 (standing 3000)", DEFAULT_MELEE_PROFILE),
            (
                "before it (standing 1000)",
                MeleeWeights {
                    standing_segment: 1_000,
                    ..DEFAULT_MELEE_PROFILE
                },
            ),
            (
                "and with no melee enclosure",
                MeleeWeights {
                    standing_segment: 1_000,
                    enclosure_turn: 0,
                    ..DEFAULT_MELEE_PROFILE
                },
            ),
        ] {
            let v = MeleeValuation::with_profiles(&profile, &DEFAULT_PROFILE);
            println!(
                "    {label:<32} {}",
                choice_with(&board, &v).map_or("-".to_owned(), heading_name_owned)
            );
        }
    }
}

fn heading_name_owned(heading: Heading) -> String {
    heading_name(heading).to_owned()
}

/// How often a weight sheet walks into a cell an equal-length rival can take.
///
/// The trap suite was harvested from games that ended walled in, so it holds no
/// head-trade positions at all and said survival was flat while iteration 19
/// raised the rate of exactly that. A suite measures what it was built from;
/// this counts the other thing directly, over ordinary play.
#[test]
#[ignore = "driven by scripts/run-trapsuite; needs a cross-section sample"]
fn measure_how_often_we_take_a_contested_cell() {
    let sample = read_harvest(&suite_path("TRAP_SUITE_SAMPLE", "sample.json"));
    let mut offered = Vec::new();
    for position in &sample {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let head = board.serpent(Seat::US).head();
        let contested = Heading::ALL.map(|h| {
            h.step(head)
                .is_some_and(|cell| contested_by_equal(&board, cell))
        });
        if contested.iter().any(|c| *c) {
            offered.push((board, contested));
        }
    }

    println!(
        "{} sampled positions where an equal-length rival shares a cell we can enter",
        offered.len()
    );
    println!("{:<34}{:>14}", "weights", "we take it");
    for (label, profile) in [
        ("iteration 19 (standing 3000)", DEFAULT_MELEE_PROFILE),
        (
            "standing 2000",
            MeleeWeights {
                standing_segment: 2_000,
                ..DEFAULT_MELEE_PROFILE
            },
        ),
        (
            "standing 1000 (before)",
            MeleeWeights {
                standing_segment: 1_000,
                ..DEFAULT_MELEE_PROFILE
            },
        ),
    ] {
        let valuation = MeleeValuation::with_profiles(&profile, &DEFAULT_PROFILE);
        let taken = offered
            .iter()
            .filter(|(board, contested)| {
                choice_with(board, &valuation).is_some_and(|h| contested[h.index()])
            })
            .count();
        println!("{label:<34}{:>9} of {:<4}", taken, offered.len());
    }
}

/// What trade risk it takes to turn a position down again.
#[test]
#[ignore = "driven by hand alongside explain_the_positions"]
fn sweep_the_trade_risk() {
    let positions = read_harvest(&suite_path("TRAP_SUITE_HARVEST", "positions.json"));
    let depth =
        u16::try_from(env_number::<u64>("TRAP_SUITE_PLAY_DEPTH", 4)).expect("a depth fits in u16");
    let valuation = MeleeValuation::standard();
    let finish = MeleeFinish::new(&DEFAULT_MELEE_PROFILE);
    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        println!("turn {} at depth {depth}", position.turn);
        for risk in [0, 8_000, 10_000, 12_000, 16_000, 24_000] {
            let mut searcher = MeleeSearcher::new(&valuation, finish).with_trade_risk(risk);
            let chosen = searcher.search_fixed(&board, depth).best;
            println!(
                "  trade risk {risk:<8} -> {}",
                chosen.map_or("-".to_owned(), heading_name_owned)
            );
        }
    }
}

/// What raising the trade risk buys and what it costs, on the same positions.
#[test]
#[ignore = "driven by scripts/run-trapsuite; needs a harvest and a sample"]
fn sweep_the_trade_risk_over_the_suite() {
    let traps = read_harvest(&suite_path("TRAP_SUITE_HARVEST", "positions.json"));
    let sample = read_harvest(&suite_path("TRAP_SUITE_SAMPLE", "sample.json"));
    let valuation = MeleeValuation::standard();

    let mut references = Vec::new();
    for position in &traps {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let reference = Heading::ALL.map(|h| rollout_survival(&board, h, ROLLOUT_SEED));
        let top = reference.iter().copied().fold(f64::MIN, f64::max);
        let low = reference.iter().copied().fold(f64::MAX, f64::min);
        if top - low >= 3.0 {
            references.push((board, reference, top));
        }
    }
    let mut contested_positions = Vec::new();
    let mut meal_positions = Vec::new();
    for position in &sample {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let head = board.serpent(Seat::US).head();
        let contested = Heading::ALL.map(|h| {
            h.step(head)
                .is_some_and(|cell| contested_by_equal(&board, cell))
        });
        if contested.iter().any(|c| *c) {
            contested_positions.push((board, contested));
        }
        let offered = meals_on_offer(&board);
        if offered.iter().any(|m| *m) {
            meal_positions.push((board, offered));
        }
    }

    println!(
        "{} trap positions, {} with a contested cell, {} with a meal on offer",
        references.len(),
        contested_positions.len(),
        meal_positions.len()
    );
    println!(
        "{:<12}{:>14}{:>16}{:>18}{:>16}",
        "trade risk", "life kept", "best step", "contested taken", "meals taken"
    );
    for risk in [8_000, 10_000, 12_000, 16_000, 24_000, 40_000] {
        let (mut kept, mut available, mut best) = (0.0f64, 0.0f64, 0usize);
        for (board, reference, top) in &references {
            available += top;
            if let Some(chosen) = choice_at_risk(board, &valuation, risk) {
                kept += reference[chosen.index()];
                best += usize::from(reference[chosen.index()] >= top - 1.0);
            }
        }
        let taken = contested_positions
            .iter()
            .filter(|(b, c)| choice_at_risk(b, &valuation, risk).is_some_and(|h| c[h.index()]))
            .count();
        let meals = meal_positions
            .iter()
            .filter(|(b, m)| choice_at_risk(b, &valuation, risk).is_some_and(|h| m[h.index()]))
            .count();
        println!(
            "{risk:<12}{:>13.1}%{:>11} of {:<4}{:>13} of {:<4}{:>11} of {:<4}",
            100.0 * kept / available,
            best,
            references.len(),
            taken,
            contested_positions.len(),
            meals,
            meal_positions.len()
        );
    }
}

/// Runs the production path -- iterative deepening under the real allowance --
/// over a file of positions, and says what depth it reached and how long it
/// took. The early stop this measures lived in the driver, not in the search,
/// so a fixed-depth reading cannot see it.
#[test]
#[ignore = "driven by hand: TRAP_SUITE_HARVEST=<positions>"]
fn explain_under_the_clock() {
    let positions = read_harvest(&suite_path("TRAP_SUITE_HARVEST", "positions.json"));
    let budget = env_number::<u64>("TRAP_SUITE_BUDGET_MS", 500);
    let valuation = MeleeValuation::standard();
    let finish = MeleeFinish::new(&DEFAULT_MELEE_PROFILE);
    let clock = SystemClock::new();

    println!(
        "{:<8}{:>8}{:>10}{:>14}{:>8}",
        "turn", "depth", "ms", "score", "move"
    );
    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let started = Instant::now();
        let mut allowance = SearchAllowance::from_request(
            &clock,
            RequestTiming {
                arrived_at: clock.now(),
            },
            Duration::from_millis(budget),
        )
        .expect("the budget must be above the reserve");
        let mut searcher = MeleeSearcher::new(&valuation, finish);
        let report = deepen_melee(&mut searcher, &board, &mut allowance, DEPTH_CEILING);
        let lost = report
            .principal_score
            .is_some_and(|score| score < 0 && score.abs() >= finish.finite_limit());
        println!(
            "{:<8}{:>8}{:>10.1}{:>14}{:>8}{:>10}{:>10}",
            position.turn,
            report.completed_depth,
            started.elapsed().as_secs_f64() * 1000.0,
            report
                .principal_score
                .map_or("-".to_owned(), |s| s.to_string()),
            report.best.map_or("-".to_owned(), heading_name_owned),
            if lost { "yes" } else { "no" },
            rollout::best_heading(&board, &mut NeverStop)
                .map_or("-".to_owned(), heading_name_owned)
        );
    }
}

/// How much of the rollout reading is the position and how much is the seed.
///
/// The rescue picks the heading with the most survival. If the gap between the
/// top two is inside the spread across seeds, it is picking noise, and one
/// arbitrary tie-break has replaced another.
#[test]
#[ignore = "driven by hand: TRAP_SUITE_HARVEST=<positions>"]
fn measure_the_rollout_noise() {
    let positions = read_harvest(&suite_path("TRAP_SUITE_HARVEST", "positions.json"));
    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        println!("turn {}", position.turn);
        for heading in Heading::ALL {
            let runs: Vec<f64> = (0..8)
                .map(|k| rollout_survival(&board, heading, ROLLOUT_SEED.wrapping_add(k * 0x9E37)))
                .collect();
            let low = runs.iter().copied().fold(f64::MAX, f64::min);
            let high = runs.iter().copied().fold(f64::MIN, f64::max);
            let mean = runs.iter().sum::<f64>() / runs.len() as f64;
            println!(
                "  {:<6} mean {mean:>6.2}   spread {low:>6.2} to {high:>6.2}",
                heading_name(heading)
            );
        }
    }
}

/// What the rescue is worth, over every harvested position the search proves
/// lost: the life the search's heading keeps against the life the rollouts'
/// heading keeps, on the same positions.
#[test]
#[ignore = "driven by scripts/run-trapsuite; needs a harvest"]
fn measure_the_rescue_on_lost_positions() {
    let positions = read_harvest(&suite_path("TRAP_SUITE_HARVEST", "positions.json"));
    let valuation = MeleeValuation::standard();
    let finish = MeleeFinish::new(&DEFAULT_MELEE_PROFILE);
    let depth =
        u16::try_from(env_number::<u64>("TRAP_SUITE_PLAY_DEPTH", 4)).expect("a depth fits in u16");

    let (mut lost, mut search_life, mut rescue_life) = (0usize, 0.0f64, 0.0f64);
    let (mut search_zero, mut rescue_zero, mut differed) = (0usize, 0usize, 0usize);
    for position in &positions {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let mut searcher = MeleeSearcher::new(&valuation, finish);
        let report = searcher.search_fixed(&board, depth);
        let is_lost = report
            .principal_score
            .is_some_and(|s| s < 0 && s.abs() >= finish.finite_limit());
        if !is_lost {
            continue;
        }
        let (Some(named), Some(rescued)) =
            (report.best, rollout::best_heading(&board, &mut NeverStop))
        else {
            continue;
        };
        lost += 1;
        differed += usize::from(named != rescued);
        let a = rollout_survival(&board, named, ROLLOUT_SEED);
        let b = rollout_survival(&board, rescued, ROLLOUT_SEED);
        search_life += a;
        rescue_life += b;
        search_zero += usize::from(a == 0.0);
        rescue_zero += usize::from(b == 0.0);
    }

    println!("positions the search proves lost: {lost}");
    if lost > 0 {
        println!("  the two pick a different heading in {differed}");
        println!(
            "  mean rollout life -- search {:.2}, rollouts {:.2}",
            search_life / lost as f64,
            rescue_life / lost as f64
        );
        println!("  headings worth nothing at all -- search {search_zero}, rollouts {rescue_zero}");
    }
}

/// Does the rescue actually get a turn? The deepening fix hands the search the
/// whole budget, and an allowance that has expired stays expired, so the two
/// changes could cancel each other out. This runs the real decision path on a
/// real clock and reports what the service says it did.
#[test]
#[ignore = "driven by hand: TRAP_SUITE_HARVEST=<positions>"]
fn explain_through_the_service() {
    let positions = read_harvest(&suite_path("TRAP_SUITE_HARVEST", "positions.json"));
    let clock = Arc::new(SystemClock::new());
    let service = VerdictService::new(clock.clone());

    println!(
        "{:<8}{:>8}{:>10}{:>8}{:>32}",
        "turn", "depth", "ms", "move", "why"
    );
    for position in &positions {
        let Ok(dto) = serde_json::from_value::<TurnRequestDto>(position.request.clone()) else {
            continue;
        };
        let started = Instant::now();
        let report = service.decide(&dto, clock.now());
        println!(
            "{:<8}{:>8}{:>10.1}{:>8}{:>32}",
            position.turn,
            report.search_depth,
            started.elapsed().as_secs_f64() * 1000.0,
            format!("{:?}", report.selected_move),
            format!("{:?}", report.selection_reason)
        );
    }
}

/// How often the search cannot tell its best two headings apart, and what the
/// rollouts say about those.
///
/// The move then falls to the fixed heading order, which is north first and has
/// nothing to do with the position. On 2026-09-25 that walked Sansón up a wall
/// into a rival coming down it, and the game was over at turn 6.
#[test]
#[ignore = "driven by scripts/run-trapsuite; needs a cross-section sample"]
fn measure_how_often_the_search_ties() {
    let sample = read_harvest(&suite_path("TRAP_SUITE_SAMPLE", "sample.json"));
    let valuation = MeleeValuation::standard();
    let finish = MeleeFinish::new(&DEFAULT_MELEE_PROFILE);
    let depth =
        u16::try_from(env_number::<u64>("TRAP_SUITE_PLAY_DEPTH", 4)).expect("a depth fits in u16");
    let margin: f64 = env_number("TRAP_SUITE_NOISE", 2.0);

    let (mut read, mut tied, mut differed, mut beyond_noise) = (0usize, 0usize, 0usize, 0usize);
    let (mut order_life, mut rollout_life) = (0.0f64, 0.0f64);
    for position in &sample {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        read += 1;
        // The exact value of every heading, with the root's own tie-break off.
        let values: Vec<(Heading, i32)> = Heading::ALL
            .into_iter()
            .filter_map(|heading| {
                let mut searcher = MeleeSearcher::new(&valuation, finish);
                searcher
                    .root_value(&board, heading, depth, &mut NeverStop)
                    .map(|value| (heading, value))
            })
            .collect();
        let Some(&(_, top)) = values.iter().max_by_key(|(_, v)| *v) else {
            continue;
        };
        let best: Vec<Heading> = values
            .iter()
            .filter(|(_, v)| *v == top)
            .map(|(h, _)| *h)
            .collect();
        if best.len() < 2 {
            continue;
        }
        tied += 1;
        // The fixed order picks the first; the rollouts pick the longest lived.
        let by_order = best[0];
        let by_rollout = best
            .iter()
            .copied()
            .max_by(|a, b| {
                rollout_survival(&board, *a, ROLLOUT_SEED).total_cmp(&rollout_survival(
                    &board,
                    *b,
                    ROLLOUT_SEED,
                ))
            })
            .expect("at least two");
        let a = rollout_survival(&board, by_order, ROLLOUT_SEED);
        let b = rollout_survival(&board, by_rollout, ROLLOUT_SEED);
        order_life += a;
        rollout_life += b;
        differed += usize::from(by_order != by_rollout);
        beyond_noise += usize::from(b - a > margin);
    }

    println!("over {read} sampled positions");
    println!(
        "  the search cannot separate its best two: {tied} ({:.0}%)",
        100.0 * tied as f64 / read as f64
    );
    if tied > 0 {
        println!("  of those, the rollouts would pick another: {differed}");
        println!("    and by more than {margin} turns of life: {beyond_noise}");
        println!(
            "  mean rollout life -- heading order {:.2}, rollouts {:.2}",
            order_life / tied as f64,
            rollout_life / tied as f64
        );
    }
}

/// Sweeps the weight of head danger: what it takes for the engine to see a
/// corridor of forced contact before it is inside one.
///
/// The trade risk is a root penalty on one ply. By the time it applies, every
/// heading is often contested and the penalty is a constant that separates
/// nothing. HeadDanger is the term that could steer away a turn earlier -- if
/// it were loud enough to be heard.
#[test]
#[ignore = "driven by hand: TRAP_SUITE_HARVEST=<positions> plus the suite sample"]
fn sweep_the_head_danger_weight() {
    let cases = read_harvest(&suite_path("TRAP_SUITE_HARVEST", "positions.json"));
    let traps = read_harvest(&suite_path("TRAP_SUITE_SUITE", "positions.json"));
    let weights: Vec<i32> = std::env::var("TRAP_SUITE_HEAD_SWEEP")
        .unwrap_or_else(|_| "300,800,1500,3000,6000".to_owned())
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();

    let mut references = Vec::new();
    for position in &traps {
        let Some(board) = board_of(&position.request) else {
            continue;
        };
        let reference = Heading::ALL.map(|h| rollout_survival(&board, h, ROLLOUT_SEED));
        let top = reference.iter().copied().fold(f64::MIN, f64::max);
        let low = reference.iter().copied().fold(f64::MAX, f64::min);
        if top - low >= 3.0 {
            references.push((board, reference, top));
        }
    }

    println!(
        "{:<10}{:>16}{:>16}   moves on the cases",
        "head", "life kept", "best step"
    );
    for weight in weights {
        let profile = MeleeWeights {
            head_danger: weight,
            ..DEFAULT_MELEE_PROFILE
        };
        let valuation = MeleeValuation::with_profiles(&profile, &DEFAULT_PROFILE);
        let (mut kept, mut available, mut best) = (0.0f64, 0.0f64, 0usize);
        for (board, reference, top) in &references {
            available += top;
            if let Some(chosen) = choice_with(board, &valuation) {
                kept += reference[chosen.index()];
                best += usize::from(reference[chosen.index()] >= top - 1.0);
            }
        }
        let moves: Vec<String> = cases
            .iter()
            .filter_map(|p| board_of(&p.request).map(|b| (p.turn, b)))
            .map(|(turn, b)| {
                format!(
                    "t{turn}:{}",
                    choice_with(&b, &valuation).map_or("-".to_owned(), heading_name_owned)
                )
            })
            .collect();
        println!(
            "{weight:<10}{:>15.1}%{:>11} of {:<4}   {}",
            100.0 * kept / available,
            best,
            references.len(),
            moves.join(" ")
        );
    }
}
