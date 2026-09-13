//! The branch-coverage gate. It reads `cargo-llvm-cov`'s JSON export
//! (`llvm.coverage.json.export`) and enforces the constitution's 90% branch floor
//! on the engine's `arena`, `valuation` and `lookahead` modules. Version 0.8.7 has
//! no `--fail-under-branches`, so the gate is ours. Line and region coverage are
//! read out as supplemental evidence and never decide the verdict.
//!
//! The parser is exercised on fixtures here; `scripts/check-branch-coverage` makes
//! the real report inside a container and runs the ignored real-report test on it.

use std::fmt::Write as _;

use serde_json::{Value, json};

/// The gated modules, as path fragments of the instrumented source files.
const GATED_MODULES: [&str; 3] = [
    "engine/src/arena/",
    "engine/src/valuation/",
    "engine/src/lookahead/",
];
#[allow(
    dead_code,
    reason = "scaffold: the parser applies it in the green step"
)]
const REQUIRED_BRANCH_PERCENT: u64 = 90;

#[derive(Debug, PartialEq)]
#[allow(
    dead_code,
    reason = "scaffold: the parser constructs these in the green step"
)]
enum GateError {
    ModuleNotFound(String),
    MalformedReport(String),
    BelowThreshold { module: String, percent: f64 },
}

/// One module's coverage, summed over its files.
#[derive(Debug, PartialEq)]
struct ModuleCoverage {
    module: String,
    branch_percent: f64,
    line_percent: f64,
    region_percent: f64,
}

/// A branch, line or region tally: how many there are and how many ran.
#[derive(Clone, Copy, Default)]
struct Tally {
    count: u128,
    covered: u128,
}

impl Tally {
    fn add(&mut self, other: Self) {
        self.count += other.count;
        self.covered += other.covered;
    }

    /// A tally with nothing to cover is fully covered.
    fn percent(self) -> f64 {
        if self.count == 0 {
            100.0
        } else {
            self.covered as f64 * 100.0 / self.count as f64
        }
    }

    /// Exact integer comparison, so 89.9% can never round up to the floor.
    fn meets_floor(self) -> bool {
        self.covered * 100 >= self.count * u128::from(REQUIRED_BRANCH_PERCENT)
    }
}

fn tally_of(file: &Value, metric: &str) -> Result<Tally, GateError> {
    let read = |field: &str| {
        file["summary"][metric][field]
            .as_u64()
            .map(u128::from)
            .ok_or_else(|| {
                GateError::MalformedReport(format!(
                    "summary.{metric}.{field} missing in {}",
                    file["filename"]
                ))
            })
    };
    Ok(Tally {
        count: read("count")?,
        covered: read("covered")?,
    })
}

/// The coverage of one module, summed over its files.
fn module_coverage(module: &str, files: &[&Value]) -> Result<(ModuleCoverage, bool), GateError> {
    let (mut branches, mut lines, mut regions) =
        (Tally::default(), Tally::default(), Tally::default());
    for file in files {
        branches.add(tally_of(file, "branches")?);
        lines.add(tally_of(file, "lines")?);
        regions.add(tally_of(file, "regions")?);
    }
    let coverage = ModuleCoverage {
        module: module.to_owned(),
        branch_percent: branches.percent(),
        line_percent: lines.percent(),
        region_percent: regions.percent(),
    };
    Ok((coverage, branches.meets_floor()))
}

/// Checks every gated module and reports every failure, not just the first.
fn check_branch_coverage(
    report: &Value,
    modules: &[&str],
) -> Result<Vec<ModuleCoverage>, Vec<GateError>> {
    let files = report["data"]
        .get(0)
        .and_then(|data| data["files"].as_array())
        .ok_or_else(|| {
            vec![GateError::MalformedReport(
                "data[0].files missing or not an array".to_owned(),
            )]
        })?;

    let mut covered = Vec::new();
    let mut errors = Vec::new();
    for module in modules {
        let members: Vec<&Value> = files
            .iter()
            .filter(|file| {
                file["filename"]
                    .as_str()
                    .is_some_and(|name| name.contains(module))
            })
            .collect();
        if members.is_empty() {
            errors.push(GateError::ModuleNotFound((*module).to_owned()));
            continue;
        }
        match module_coverage(module, &members) {
            Ok((coverage, true)) => covered.push(coverage),
            Ok((coverage, false)) => errors.push(GateError::BelowThreshold {
                module: coverage.module,
                percent: coverage.branch_percent,
            }),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(covered)
    } else {
        Err(errors)
    }
}

// ---- fixtures -----------------------------------------------------------------

struct Counts {
    count: u64,
    covered: u64,
}

fn metric(counts: &Counts) -> Value {
    json!({ "count": counts.count, "covered": counts.covered, "notcovered": counts.count - counts.covered,
            "percent": if counts.count == 0 { 0.0 } else { counts.covered as f64 * 100.0 / counts.count as f64 } })
}

fn file(name: &str, branches: (u64, u64), lines: (u64, u64), regions: (u64, u64)) -> Value {
    let counts = |(count, covered)| Counts { count, covered };
    json!({
        "filename": name,
        "summary": {
            "branches": metric(&counts(branches)),
            "lines": metric(&counts(lines)),
            "regions": metric(&counts(regions)),
            "functions": { "count": 1, "covered": 1, "percent": 100.0 }
        }
    })
}

fn report(files: Vec<Value>) -> Value {
    json!({ "type": "llvm.coverage.json.export", "version": "2.0.1", "data": [{ "files": files }] })
}

/// A file in each gated module with the given branch counts, everything else full.
fn healthy(arena: (u64, u64), valuation: (u64, u64), lookahead: (u64, u64)) -> Value {
    report(vec![
        file("/w/engine/src/arena/duel.rs", arena, (10, 10), (10, 10)),
        file(
            "/w/engine/src/valuation/mod.rs",
            valuation,
            (10, 10),
            (10, 10),
        ),
        file(
            "/w/engine/src/lookahead/minimax.rs",
            lookahead,
            (10, 10),
            (10, 10),
        ),
    ])
}

fn gate(report: &Value) -> Result<Vec<ModuleCoverage>, Vec<GateError>> {
    check_branch_coverage(report, &GATED_MODULES)
}

// ---- the threshold ---------------------------------------------------------------

#[test]
fn a_module_below_ninety_percent_is_rejected_and_named() {
    let result = gate(&healthy((20, 17), (20, 20), (20, 20)));

    let errors = result.expect_err("85% is below the floor");
    assert_eq!(
        errors,
        vec![GateError::BelowThreshold {
            module: "engine/src/arena/".to_owned(),
            percent: 85.0
        }]
    );
}

#[test]
fn exactly_ninety_percent_is_accepted() {
    let result = gate(&healthy((20, 18), (10, 9), (100, 90)));

    let modules = result.expect("90% meets the floor");
    assert!(
        modules
            .iter()
            .all(|m| (m.branch_percent - 90.0).abs() < 1e-9)
    );
}

#[test]
fn above_ninety_percent_is_accepted() {
    assert!(gate(&healthy((40, 39), (20, 20), (200, 190))).is_ok());
}

#[test]
fn the_comparison_is_exact_just_under_the_floor() {
    // 89.9% must fail and 90.0% must pass; floating point must not blur it.
    assert!(gate(&healthy((1000, 899), (10, 10), (10, 10))).is_err());
    assert!(gate(&healthy((1000, 900), (10, 10), (10, 10))).is_ok());
}

#[test]
fn files_are_weighted_by_their_branch_count_not_averaged() {
    // 90 of 90 plus 0 of 10 is 90% overall although one file is at 0%.
    let report = report(vec![
        file("/w/engine/src/arena/big.rs", (90, 90), (10, 10), (10, 10)),
        file("/w/engine/src/arena/small.rs", (10, 0), (10, 10), (10, 10)),
        file("/w/engine/src/valuation/v.rs", (10, 10), (10, 10), (10, 10)),
        file("/w/engine/src/lookahead/l.rs", (10, 10), (10, 10), (10, 10)),
    ]);

    let modules = gate(&report).expect("weighted 90% passes");

    assert!((modules[0].branch_percent - 90.0).abs() < 1e-9);
}

#[test]
fn a_module_without_branch_points_counts_as_fully_covered() {
    let result = gate(&healthy((0, 0), (10, 10), (10, 10)));

    let modules = result.expect("nothing to cover");
    assert!((modules[0].branch_percent - 100.0).abs() < 1e-9);
}

#[test]
fn every_failing_module_is_reported() {
    let errors = gate(&healthy((20, 10), (20, 10), (20, 19))).expect_err("two are below");

    assert_eq!(errors.len(), 2);
    assert!(errors.iter().any(
        |e| matches!(e, GateError::BelowThreshold { module, .. } if module.contains("arena"))
    ));
    assert!(errors.iter().any(
        |e| matches!(e, GateError::BelowThreshold { module, .. } if module.contains("valuation"))
    ));
}

// ---- what does not count ------------------------------------------------------------

#[test]
fn files_outside_the_gated_modules_never_affect_the_verdict() {
    let mut files = vec![
        file("/w/engine/src/arena/duel.rs", (20, 20), (10, 10), (10, 10)),
        file(
            "/w/engine/src/valuation/mod.rs",
            (20, 20),
            (10, 10),
            (10, 10),
        ),
        file(
            "/w/engine/src/lookahead/minimax.rs",
            (20, 20),
            (10, 10),
            (10, 10),
        ),
    ];
    files.push(file(
        "/w/engine/src/gateway/http.rs",
        (50, 0),
        (10, 0),
        (10, 0),
    ));
    files.push(file(
        "/w/engine/tests/support/mod.rs",
        (50, 0),
        (10, 0),
        (10, 0),
    ));
    files.push(file("/w/engine/src/main.rs", (2, 0), (10, 0), (10, 0)));

    assert!(gate(&report(files)).is_ok());
}

#[test]
fn line_and_region_percentages_are_reported_but_never_decide() {
    let poor_lines = report(vec![
        file("/w/engine/src/arena/a.rs", (20, 19), (10, 1), (10, 2)),
        file("/w/engine/src/valuation/v.rs", (20, 19), (10, 1), (10, 2)),
        file("/w/engine/src/lookahead/l.rs", (20, 19), (10, 1), (10, 2)),
    ]);
    let good_lines_bad_branches = healthy((20, 10), (20, 20), (20, 20));

    let modules = gate(&poor_lines).expect("branches pass, so the gate passes");

    assert!((modules[0].line_percent - 10.0).abs() < 1e-9);
    assert!((modules[0].region_percent - 20.0).abs() < 1e-9);
    assert!(gate(&good_lines_bad_branches).is_err());
}

// ---- unusable reports ----------------------------------------------------------------

#[test]
fn a_gated_module_missing_from_the_report_is_an_error() {
    let report = report(vec![
        file("/w/engine/src/arena/a.rs", (20, 20), (10, 10), (10, 10)),
        file("/w/engine/src/valuation/v.rs", (20, 20), (10, 10), (10, 10)),
    ]);

    let errors = gate(&report).expect_err("lookahead was not instrumented");

    assert_eq!(
        errors,
        vec![GateError::ModuleNotFound(
            "engine/src/lookahead/".to_owned()
        )]
    );
}

#[test]
fn a_malformed_report_is_an_error_not_a_pass() {
    for broken in [
        json!({}),
        json!({ "data": [] }),
        json!({ "data": [{ "files": "no" }] }),
    ] {
        let errors = gate(&broken).expect_err("nothing to read");

        assert!(
            matches!(errors[0], GateError::MalformedReport(_)),
            "{errors:?}"
        );
    }
    let missing_metric = report(vec![
        json!({ "filename": "/w/engine/src/arena/a.rs", "summary": {} }),
    ]);
    assert!(matches!(
        gate(&missing_metric).unwrap_err()[0],
        GateError::MalformedReport(_)
    ));
}

// ---- the real report --------------------------------------------------------------------

/// Run by `scripts/check-branch-coverage` on the report from the container.
#[test]
#[ignore = "needs COVERAGE_REPORT_PATH from scripts/check-branch-coverage"]
fn the_real_engine_report_meets_the_branch_threshold() {
    let path = std::env::var("COVERAGE_REPORT_PATH").expect("COVERAGE_REPORT_PATH is set");
    let text = std::fs::read_to_string(&path).expect("the report is readable");
    let report: Value = serde_json::from_str(&text).expect("the report is JSON");

    let result = check_branch_coverage(&report, &GATED_MODULES);

    let mut table = String::from("module | branches | lines | regions\n");
    if let Ok(modules) = &result {
        for m in modules {
            let _ = writeln!(
                table,
                "{} | {:.2}% | {:.2}% | {:.2}%",
                m.module, m.branch_percent, m.line_percent, m.region_percent
            );
        }
    }
    println!("{table}");
    assert!(result.is_ok(), "{result:?}");
}
