//! `spar`: runs the seeded benchmark of the engine and the sibling baseline
//! against each opponent of a roster through the official rules CLI and writes a
//! versioned report; with `--melee`, the four-snake placement benchmark instead.

use std::process::{Command, ExitCode};
use std::time::Duration;

use tiger_sparring::benchmark::{Plan, run_benchmark, run_melee_benchmark};
use tiger_sparring::launcher::ProcessLauncher;
use tiger_sparring::ledger::{EngineIdentity, EnvironmentIdentity, JsonFileSink};
use tiger_sparring::options::Options;
use tiger_sparring::roster::Roster;
use tiger_sparring::runner::OfficialCli;
use tiger_sparring::statistics::wilson_interval;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match Options::parse(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!(
                "spar: {error:?}\nusage: spar --output REPORT.json [--melee] [--roster PATH] [--oracle PATH] [--seeds 1-30] [--workers N] [--overwrite] [--commit SHA] [--scratch DIR]"
            );
            return ExitCode::FAILURE;
        }
    };
    let roster = match Roster::load(&options.roster) {
        Ok(roster) => roster,
        Err(error) => {
            eprintln!(
                "spar: cannot use the roster {}: {error:?}",
                options.roster.display()
            );
            return ExitCode::FAILURE;
        }
    };

    let engine = EngineIdentity {
        name: "tiger-engine".to_owned(),
        commit: options
            .commit
            .clone()
            .unwrap_or_else(|| capture("git", &["rev-parse", "HEAD"])),
    };
    let environment = EnvironmentIdentity {
        cpu: cpu_model(),
        kernel: capture("uname", &["-srm"]),
        rustc: capture("rustc", &["--version"]),
        rules_cli_release: roster.rules_cli_release.clone(),
    };
    let plan = Plan {
        seeds: options.seeds.clone(),
        workers: options.workers,
    };
    let runner = OfficialCli::new(options.oracle.clone(), options.scratch.clone());
    let launcher = ProcessLauncher::new(Duration::from_secs(30));
    let sink = JsonFileSink::new(options.output.clone(), options.overwrite);

    println!(
        "spar: {} seeds, {} worker(s), {}: {}",
        plan.seeds.len(),
        plan.workers,
        if options.melee { "seats" } else { "opponents" },
        roster
            .opponents
            .iter()
            .map(|o| o.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if options.melee {
        return melee(
            &roster,
            &plan,
            &runner,
            &launcher,
            &sink,
            engine,
            environment,
            &options,
        );
    }
    let outcome = match run_benchmark(
        &roster,
        &plan,
        &runner,
        &launcher,
        &sink,
        engine,
        environment,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("spar: the benchmark failed: {error:?}");
            return ExitCode::FAILURE;
        }
    };

    println!("challenger | opponent | wins | losses | draws | win rate | Wilson 95% | mean turns");
    for m in &outcome.matchups {
        let (lo, hi) = wilson_interval(m.wins, m.wins + m.losses + m.draws).unwrap_or((0.0, 1.0));
        println!(
            "{} | {} | {} | {} | {} | {:.3} | [{lo:.3}, {hi:.3}] | {:.1}",
            m.challenger, m.opponent, m.wins, m.losses, m.draws, m.win_rate, m.mean_turns
        );
    }
    for v in &outcome.verdicts {
        println!(
            "against {}: challenger {:.3} vs baseline {:.3} -> {}",
            v.opponent,
            v.challenger_win_rate,
            v.baseline_win_rate,
            if v.beats_baseline {
                "beats the baseline"
            } else {
                "does NOT beat the baseline"
            }
        );
    }
    println!("report written to {}", options.output.display());
    if outcome.criterion_met() {
        println!("criterion 8: MET");
        ExitCode::SUCCESS
    } else {
        println!("criterion 8: NOT MET");
        ExitCode::from(2)
    }
}

/// The placement benchmark: prints every game's placements and the means, and
/// exits 0 when criterion 6 holds.
#[allow(clippy::too_many_arguments)]
fn melee(
    roster: &Roster,
    plan: &Plan,
    runner: &OfficialCli,
    launcher: &ProcessLauncher,
    sink: &JsonFileSink,
    engine: EngineIdentity,
    environment: EnvironmentIdentity,
    options: &Options,
) -> ExitCode {
    let outcome =
        match run_melee_benchmark(roster, plan, runner, launcher, sink, engine, environment) {
            Ok(outcome) => outcome,
            Err(error) => {
                eprintln!("spar: the benchmark failed: {error:?}");
                return ExitCode::FAILURE;
            }
        };
    println!("seat one | seed | turns | placements");
    for game in &outcome.report.games {
        let places: Vec<String> = game
            .placements
            .iter()
            .map(|(snake, place)| format!("{snake} {place}"))
            .collect();
        println!(
            "{} | {} | {} | {}",
            game.seat_one,
            game.seed,
            game.turns,
            places.join(", ")
        );
    }
    let summary = &outcome.report.summary;
    println!(
        "mean placement: {} {:.3} | {} {:.3} | {} (in the {}'s games) {:.3}",
        summary.challenger,
        summary.challenger_mean,
        summary.baseline,
        summary.baseline_mean,
        summary.reference_opponent,
        summary.challenger,
        summary.reference_mean_in_challenger_games
    );
    for opponent in &summary.opponents {
        println!(
            "{}: {:.3} beside the challenger, {:.3} beside the baseline",
            opponent.id, opponent.mean_with_challenger, opponent.mean_with_baseline
        );
    }
    println!("report written to {}", options.output.display());
    if outcome.criterion_met() {
        println!("criterion 6: MET");
        ExitCode::SUCCESS
    } else {
        println!("criterion 6: NOT MET");
        ExitCode::from(2)
    }
}

/// The trimmed stdout of a command, or "unknown" when it cannot be run.
fn capture(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("model name"))
                .and_then(|line| line.split_once(':'))
                .map(|(_, model)| model.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned())
}
