use std::collections::BTreeMap;
use std::fs;

use tiger_sparring::roster::{Roster, RosterError};

const ROSTER: &str = r##"{
  "rules_cli_release": "v1.2.3",
  "challenger": { "id": "tiger", "launch": {
      "program": "/repo/target/release/tiger-engine",
      "env": { "BIND_ADDR": "127.0.0.1", "PORT": "{port}" } } },
  "baseline": { "id": "baseline", "launch": {
      "program": "/repo/target/baseline/release/battle-snake-rust",
      "env": { "BIND_ADDR": "127.0.0.1", "PORT": "{port}" } } },
  "opponents": [
    { "id": "flood", "launch": {
        "program": "/repo/reference/snork/target/release/server",
        "args": ["--host", "127.0.0.1:{port}", "--config", "{\"Flood\":{}}"] } },
    { "id": "shapeshifter", "launch": {
        "program": "/repo/reference/ss/release/shapeshifter",
        "env": { "PORT": "{port}" } } }
  ]
}"##;

#[test]
fn a_roster_names_the_challenger_the_baseline_and_the_opponents_in_order() {
    let roster = Roster::from_json(ROSTER).expect("a valid roster");

    assert_eq!(roster.rules_cli_release, "v1.2.3");
    assert_eq!(roster.challenger.id, "tiger");
    assert_eq!(roster.baseline.id, "baseline");
    let ids: Vec<&str> = roster.opponents.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["flood", "shapeshifter"]);
}

#[test]
fn a_launch_puts_the_chosen_port_into_arguments_and_environment() {
    let roster = Roster::from_json(ROSTER).unwrap();

    let flood = roster.opponents[0].launch.command(4321);
    let tiger = roster.challenger.launch.command(8080);

    assert_eq!(flood.program, "/repo/reference/snork/target/release/server");
    assert_eq!(
        flood.args,
        ["--host", "127.0.0.1:4321", "--config", "{\"Flood\":{}}"]
    );
    assert!(flood.env.is_empty());
    assert_eq!(
        tiger.env,
        BTreeMap::from([
            ("BIND_ADDR".to_owned(), "127.0.0.1".to_owned()),
            ("PORT".to_owned(), "8080".to_owned())
        ])
    );
    assert!(tiger.args.is_empty(), "arguments default to none");
}

#[test]
fn a_malformed_roster_is_refused_with_the_reason() {
    assert!(matches!(
        Roster::from_json("nope"),
        Err(RosterError::NotJson(_))
    ));

    let without = |field: &str| {
        let mut value: serde_json::Value = serde_json::from_str(ROSTER).unwrap();
        value.as_object_mut().unwrap().remove(field);
        value.to_string()
    };
    assert_eq!(
        Roster::from_json(&without("challenger")),
        Err(RosterError::Missing("challenger"))
    );
    assert_eq!(
        Roster::from_json(&without("baseline")),
        Err(RosterError::Missing("baseline"))
    );
    assert_eq!(
        Roster::from_json(&without("rules_cli_release")),
        Err(RosterError::Missing("rules_cli_release"))
    );
    assert_eq!(
        Roster::from_json(&without("opponents")),
        Err(RosterError::Missing("opponents"))
    );
}

#[test]
fn a_roster_needs_opponents_distinct_ids_and_real_programs() {
    let mut value: serde_json::Value = serde_json::from_str(ROSTER).unwrap();

    value["opponents"] = serde_json::json!([]);
    assert_eq!(
        Roster::from_json(&value.to_string()),
        Err(RosterError::NoOpponents)
    );

    let mut value: serde_json::Value = serde_json::from_str(ROSTER).unwrap();
    value["opponents"][1]["id"] = serde_json::json!("flood");
    assert_eq!(
        Roster::from_json(&value.to_string()),
        Err(RosterError::DuplicateId("flood".to_owned()))
    );

    let mut value: serde_json::Value = serde_json::from_str(ROSTER).unwrap();
    value["opponents"][0]["id"] = serde_json::json!("tiger");
    assert_eq!(
        Roster::from_json(&value.to_string()),
        Err(RosterError::DuplicateId("tiger".to_owned()))
    );

    let mut value: serde_json::Value = serde_json::from_str(ROSTER).unwrap();
    value["baseline"]["launch"]["program"] = serde_json::json!("");
    assert_eq!(
        Roster::from_json(&value.to_string()),
        Err(RosterError::EmptyProgram("baseline".to_owned()))
    );
}

#[test]
fn a_roster_file_is_loaded_and_a_missing_one_is_an_error() {
    let dir = std::env::temp_dir().join(format!("tiger-roster-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("roster.json");
    fs::write(&path, ROSTER).unwrap();

    let loaded = Roster::load(&path);
    let missing = Roster::load(&dir.join("absent.json"));
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(loaded.unwrap().opponents.len(), 2);
    assert!(matches!(missing, Err(RosterError::Unreadable(_))));
}
