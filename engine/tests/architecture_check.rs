//! Self-tests for the quality gates: the architecture check, the verify runner
//! and the dependency policy. Each plants a tree in a scratch directory and runs
//! the real script on it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the engine crate sits inside the repository")
        .to_path_buf()
}

/// A scratch source tree that is removed when dropped.
struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "tiger-architecture-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&root).expect("a scratch directory");
        Self { root }
    }

    fn file(&self, relative: &str, contents: &str) -> &Self {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
        self
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Verdict {
    passed: bool,
    stdout: String,
    stderr: String,
}

fn verdict(output: &Output) -> Verdict {
    Verdict {
        passed: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn check(dir: &Path) -> Verdict {
    let output = Command::new("bash")
        .arg(repo_root().join("scripts/check-architecture"))
        .arg(dir)
        .output()
        .expect("bash runs the architecture check");
    verdict(&output)
}

fn check_one(relative: &str, contents: &str) -> Verdict {
    let tree = Tree::new();
    tree.file(relative, contents);
    check(&tree.root)
}

const GUARDED: [&str; 4] = ["arena", "valuation", "lookahead", "verdict"];

// ---- the architecture check ---------------------------------------------------

#[test]
fn the_real_source_tree_respects_the_inward_dependencies() {
    let result = check(&repo_root().join("engine/src"));

    assert!(result.passed, "{}", result.stderr);
}

#[test]
fn a_forbidden_import_fails_in_every_guarded_layer_and_names_file_and_rule() {
    let forbidden = [
        ("use axum::Router;", "axum"),
        ("use tokio::task;", "tokio"),
        ("use tracing::info;", "tracing"),
        ("use std::fs;", "std::fs"),
        ("use std::process::Command;", "std::process"),
        ("use crate::gateway::beacon;", "gateway"),
    ];

    for layer in GUARDED {
        for (line, rule) in forbidden {
            let file = format!("{layer}/planted.rs");

            let result = check_one(&file, &format!("{line}\n"));

            assert!(!result.passed, "{layer}: `{line}` must fail");
            assert!(
                result.stderr.contains(&file),
                "{layer}/{rule}: {}",
                result.stderr
            );
            assert!(
                result.stderr.contains(rule),
                "{layer}/{rule}: {}",
                result.stderr
            );
        }
    }
}

#[test]
fn grouped_multi_line_and_inline_uses_are_caught_too() {
    let cases = [
        ("verdict/a.rs", "use std::{fmt, fs};\n"),
        ("verdict/b.rs", "use std::{\n    fmt,\n    process,\n};\n"),
        ("verdict/c.rs", "use crate::{arena, gateway};\n"),
        (
            "lookahead/d.rs",
            "fn f() { let _ = tokio::spawn(async {}); }\n",
        ),
        ("arena/e.rs", "fn f() { let _ = std::fs::read(\"x\"); }\n"),
        ("valuation/f.rs", "fn f() { tracing::info!(\"x\"); }\n"),
        (
            "valuation/g.rs",
            "fn f() { let _ = crate::gateway::beacon::SCHEMA_VERSION; }\n",
        ),
    ];

    for (file, contents) in cases {
        let result = check_one(file, contents);

        assert!(!result.passed, "{file}: {contents:?} must fail");
    }
}

#[test]
fn comments_and_look_alike_names_do_not_trigger_the_check() {
    let tree = Tree::new();
    tree.file(
        "arena/quiet.rs",
        "//! Mentions axum, tokio, tracing, std::fs and crate::gateway in prose.\n\
         // use std::process::Command;\n\
         use std::fmt;\n\
         use core::time::Duration;\n\
         use crate::rules_core::Clock;\n\
         struct Processed; struct TokioLike; fn tracing_level() {}\n",
    );

    let result = check(&tree.root);

    assert!(result.passed, "{}", result.stderr);
}

#[test]
fn layers_may_only_import_inward() {
    let upward = [
        ("arena/x.rs", "use crate::valuation::Assessor;\n"),
        ("arena/x.rs", "use crate::lookahead::minimax::Searcher;\n"),
        ("arena/x.rs", "use crate::verdict::route::Route;\n"),
        (
            "valuation/x.rs",
            "use crate::lookahead::minimax::Searcher;\n",
        ),
        ("valuation/x.rs", "use crate::verdict::route::Route;\n"),
        (
            "lookahead/x.rs",
            "use crate::verdict::report::VerdictReport;\n",
        ),
    ];
    for (file, contents) in upward {
        let result = check_one(file, contents);

        assert!(!result.passed, "{file}: {contents:?} imports outward");
    }

    let tree = Tree::new();
    tree.file("valuation/y.rs", "use crate::arena::duel::DuelBoard;\n")
        .file(
            "lookahead/y.rs",
            "use crate::valuation::Assessor;\nuse crate::arena::duel::DuelBoard;\n",
        )
        .file(
            "verdict/y.rs",
            "use crate::lookahead::minimax::Searcher;\nuse crate::valuation::finish::Finish;\n",
        );
    let inward = check(&tree.root);
    assert!(inward.passed, "{}", inward.stderr);
}

#[test]
fn the_gateway_and_the_files_beside_the_layers_are_not_restricted() {
    let tree = Tree::new();
    tree.file(
        "gateway/http.rs",
        "use axum::Router;\nuse tokio::task;\nuse tracing::info;\nuse std::fs;\nuse crate::verdict::service::VerdictService;\n",
    )
    .file("lib.rs", "use axum::Router;\nuse crate::gateway::beacon::DecisionBeacon;\n")
    .file("main.rs", "use tokio::net::TcpListener;\n")
    .file("rules_core.rs", "pub use battle_snake_rust::domain::state::TurnState;\n");

    let result = check(&tree.root);

    assert!(result.passed, "{}", result.stderr);
}

#[test]
fn every_violation_is_reported_not_just_the_first() {
    let tree = Tree::new();
    tree.file("arena/one.rs", "use axum::Router;\n")
        .file("verdict/two.rs", "use std::fs;\n");

    let result = check(&tree.root);

    assert!(!result.passed);
    assert!(result.stderr.contains("arena/one.rs"), "{}", result.stderr);
    assert!(
        result.stderr.contains("verdict/two.rs"),
        "{}",
        result.stderr
    );
}

#[test]
fn a_missing_source_directory_is_an_error_not_a_silent_pass() {
    let missing = std::env::temp_dir().join("tiger-architecture-does-not-exist");

    let result = check(&missing);

    assert!(!result.passed);
    assert!(
        result.stderr.contains("no such directory"),
        "{}",
        result.stderr
    );
}

// ---- the verify runner -----------------------------------------------------------

fn verify(stages: &[&str], architecture_dir: Option<&Path>) -> Verdict {
    let mut command = Command::new("bash");
    command
        .arg(repo_root().join("scripts/verify"))
        .args(stages)
        .current_dir(repo_root());
    if let Some(dir) = architecture_dir {
        command.env("ARCH_SRC", dir);
    }
    verdict(&command.output().expect("bash runs the verify script"))
}

#[test]
fn verify_runs_a_named_stage_and_says_so() {
    let result = verify(&["architecture"], None);

    assert!(result.passed, "{}", result.stderr);
    assert!(
        result.stdout.contains("==> architecture check"),
        "{}",
        result.stdout
    );
}

#[test]
fn a_failing_stage_exits_non_zero_and_names_the_stage() {
    let tree = Tree::new();
    tree.file("arena/planted.rs", "use axum::Router;\n");

    let result = verify(&["architecture"], Some(&tree.root));

    assert!(!result.passed);
    assert!(
        result
            .stderr
            .contains("verify: FAILED at stage: architecture check"),
        "{}",
        result.stderr
    );
}

#[test]
fn an_unknown_stage_is_refused() {
    let result = verify(&["nonsense"], None);

    assert!(!result.passed);
    assert!(result.stderr.contains("unknown stage"), "{}", result.stderr);
}

#[test]
fn the_default_run_lists_every_stage_in_order() {
    let result = verify(&["--list"], None);

    assert!(result.passed, "{}", result.stderr);
    let listed: Vec<&str> = result.stdout.lines().collect();
    assert_eq!(listed, ["fmt", "test", "clippy", "architecture", "deny"]);
}

// ---- the dependency policy -----------------------------------------------------------

#[test]
fn deny_toml_states_every_policy_explicitly() {
    let text = fs::read_to_string(repo_root().join("deny.toml")).expect("deny.toml exists");

    for section in ["[advisories]", "[licenses]", "[bans]", "[sources]"] {
        assert!(text.contains(section), "missing {section}");
    }
    for setting in [
        "yanked = \"deny\"",
        "unknown-registry = \"deny\"",
        "unknown-git = \"deny\"",
        "wildcards = \"deny\"",
    ] {
        assert!(text.contains(setting), "missing {setting}");
    }
    assert!(
        text.contains("allow = ["),
        "the license allow-list is explicit"
    );
}
