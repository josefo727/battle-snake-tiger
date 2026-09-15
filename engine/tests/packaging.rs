//! The container packaging promises (spec criteria 2 and 6, constitution Article XIII),
//! checked against the files themselves; `scripts/container-smoke` builds and runs the
//! image for real.

use std::fs;
use std::path::{Path, PathBuf};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read(relative: &str) -> String {
    fs::read_to_string(repository().join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

fn instructions(text: &str, keyword: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|w| w.eq_ignore_ascii_case(keyword))
        })
        .map(str::to_owned)
        .collect()
}

fn digests(dockerfile: &str) -> Vec<String> {
    instructions(dockerfile, "FROM")
        .iter()
        .filter_map(|line| {
            line.split('@')
                .nth(1)
                .map(|rest| rest.split_whitespace().next().unwrap().to_owned())
        })
        .collect()
}

#[test]
fn both_images_are_pinned_by_digest_to_the_same_ones_the_sibling_trusts() {
    let ours = read("Dockerfile");
    let sibling = fs::read_to_string(repository().join("../battle-snake-rust/Dockerfile"))
        .expect("the sibling Dockerfile");

    assert_eq!(
        instructions(&ours, "FROM").len(),
        2,
        "a builder and a runtime stage"
    );
    assert_eq!(digests(&ours).len(), 2, "every FROM carries a digest");
    assert!(
        digests(&ours)
            .iter()
            .all(|d| d.starts_with("sha256:") && d.len() == 71)
    );
    assert_eq!(digests(&ours), digests(&sibling));
}

#[test]
fn the_builder_builds_only_the_engine_from_the_lockfile() {
    let dockerfile = read("Dockerfile");

    let runs = instructions(&dockerfile, "RUN");
    assert!(
        runs.iter()
            .any(|r| r.contains("cargo build --release --locked -p tiger-engine")),
        "{runs:?}"
    );
}

#[test]
fn the_sibling_rules_core_arrives_as_a_named_context_carrying_no_build_output() {
    let dockerfile = read("Dockerfile");

    let copies = instructions(&dockerfile, "COPY");
    let from_sibling: Vec<&String> = copies
        .iter()
        .filter(|c| c.contains("--from=sibling"))
        .collect();
    assert!(
        !from_sibling.is_empty(),
        "the sibling is a named build context: {copies:?}"
    );
    for copy in from_sibling {
        assert!(!copy.contains("target"), "{copy}");
        assert!(
            copy.contains("/battle-snake-rust"),
            "it lands where the engine's path dependency points: {copy}"
        );
    }
}

#[test]
fn the_runtime_runs_as_an_unprivileged_user_with_only_the_engine_binary() {
    let dockerfile = read("Dockerfile");

    let users = instructions(&dockerfile, "USER");
    assert_eq!(users.len(), 1);
    assert!(
        !users[0].contains("root") && !users[0].ends_with(" 0"),
        "{users:?}"
    );
    assert!(
        dockerfile.contains("--uid 10001"),
        "the same unprivileged uid the sibling uses"
    );
    let builder_copies: Vec<String> = instructions(&dockerfile, "COPY")
        .into_iter()
        .filter(|c| c.contains("--from=builder"))
        .collect();
    assert_eq!(builder_copies.len(), 1, "{builder_copies:?}");
    assert!(builder_copies[0].contains("target/release/tiger-engine"));
    assert_eq!(instructions(&dockerfile, "ENTRYPOINT").len(), 1);
    assert!(instructions(&dockerfile, "ENTRYPOINT")[0].contains("/usr/local/bin/tiger-engine"));
}

#[test]
fn the_defaults_listen_on_every_interface_at_8080() {
    let dockerfile = read("Dockerfile");

    assert!(dockerfile.contains("BIND_ADDR=0.0.0.0"));
    assert!(dockerfile.contains("PORT=8080"));
    assert!(
        instructions(&dockerfile, "EXPOSE")
            .iter()
            .any(|e| e.contains("8080"))
    );
}

#[test]
fn opponents_the_rules_cli_and_build_output_never_enter_the_build_context() {
    let ignore = read(".dockerignore");
    let dockerfile = read("Dockerfile");

    let lines: Vec<&str> = ignore.lines().map(str::trim).collect();
    for excluded in [
        "reference",
        ".rules-oracle",
        "target",
        ".git",
        ".specs",
        ".claude",
    ] {
        assert!(
            lines.iter().any(
                |l| l.trim_start_matches('/').trim_end_matches('/') == excluded
                    || l.trim_start_matches("**/").trim_end_matches('/') == excluded
            ),
            "{excluded} must be ignored: {lines:?}"
        );
    }
    for copy in instructions(&dockerfile, "COPY")
        .iter()
        .chain(instructions(&dockerfile, "ADD").iter())
    {
        for forbidden in [
            "reference",
            ".rules-oracle",
            "target/baseline",
            "sparring/src/main.rs",
        ] {
            assert!(!copy.contains(forbidden), "{copy}");
        }
    }
}

#[test]
fn the_smoke_script_checks_every_promise_of_the_image() {
    let script = read("scripts/container-smoke");

    for promise in [
        "--read-only",
        "id -u",
        r##"{"apiversion":"1","author":"josefo727","color":"#00D5FF","head":"tiger-king","tail":"tiger-tail","version":"0.1.0"}"##,
        "/start",
        "/end",
        "/move",
        "duel_search",
        "safety_fallback",
        "shapeshifter",
        "battlesnake",
        "--build-context",
    ] {
        assert!(
            script.contains(promise),
            "the smoke script never checks `{promise}`"
        );
    }
    let mode = std::os::unix::fs::PermissionsExt::mode(
        &fs::metadata(repository().join("scripts/container-smoke"))
            .unwrap()
            .permissions(),
    );
    assert!(mode & 0o111 != 0, "the script is executable");
}

// ---- the log directory on the shared server -----------------------------------------------

#[test]
fn the_smoke_script_runs_the_image_with_a_mounted_log_directory_and_reads_todays_file() {
    let script = read("scripts/container-smoke");

    for promise in [
        "LOG_DIR=/var/log/tiger",
        "tiger.log.",
        "game_started",
        "game_ended",
        "move_decision",
    ] {
        assert!(
            script.contains(promise),
            "the smoke script never checks `{promise}`"
        );
    }
}
