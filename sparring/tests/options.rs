use std::path::PathBuf;

use tiger_sparring::options::{Options, OptionsError};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn only_the_output_is_required_and_everything_else_has_a_default() {
    let options = Options::parse(&args(&["--output", "report.json"])).expect("valid");

    assert_eq!(options.output, PathBuf::from("report.json"));
    assert_eq!(options.roster, PathBuf::from("reference/roster.json"));
    assert_eq!(options.oracle, PathBuf::from(".rules-oracle/battlesnake"));
    assert_eq!(options.scratch, PathBuf::from("target/sparring"));
    assert_eq!(options.seeds, (1..=30).collect::<Vec<u64>>());
    assert_eq!(options.workers, 1);
    assert!(!options.overwrite);
    assert_eq!(options.commit, None);
    assert!(!options.melee);
}

#[test]
fn the_melee_flag_selects_the_placement_benchmark_and_its_roster() {
    let melee = Options::parse(&args(&["--output", "m.json", "--melee"])).expect("valid");
    let own_roster = Options::parse(&args(&[
        "--melee", "--roster", "r.json", "--output", "m.json",
    ]))
    .expect("valid");

    assert!(melee.melee);
    assert_eq!(melee.roster, PathBuf::from("reference/roster-melee.json"));
    assert!(own_roster.melee);
    assert_eq!(own_roster.roster, PathBuf::from("r.json"));
}

#[test]
fn every_flag_is_read() {
    let options = Options::parse(&args(&[
        "--output",
        "o.json",
        "--roster",
        "r.json",
        "--oracle",
        "cli",
        "--scratch",
        "s",
        "--seeds",
        "5-8",
        "--workers",
        "4",
        "--overwrite",
        "--commit",
        "abcdef1",
    ]))
    .expect("valid");

    assert_eq!(options.roster, PathBuf::from("r.json"));
    assert_eq!(options.oracle, PathBuf::from("cli"));
    assert_eq!(options.scratch, PathBuf::from("s"));
    assert_eq!(options.seeds, [5, 6, 7, 8]);
    assert_eq!(options.workers, 4);
    assert!(options.overwrite);
    assert_eq!(options.commit.as_deref(), Some("abcdef1"));
}

#[test]
fn seeds_may_be_one_number_or_a_range() {
    assert_eq!(
        Options::parse(&args(&["--output", "o", "--seeds", "7"]))
            .unwrap()
            .seeds,
        [7]
    );
    assert_eq!(
        Options::parse(&args(&["--output", "o", "--seeds", "1-3"]))
            .unwrap()
            .seeds,
        [1, 2, 3]
    );
}

#[test]
fn a_missing_output_an_unknown_flag_and_a_dangling_flag_are_refused() {
    assert_eq!(Options::parse(&args(&[])), Err(OptionsError::MissingOutput));
    assert_eq!(
        Options::parse(&args(&["--output", "o", "--fast"])),
        Err(OptionsError::UnknownFlag("--fast".to_owned()))
    );
    assert_eq!(
        Options::parse(&args(&["--output"])),
        Err(OptionsError::MissingValue("--output".to_owned()))
    );
}

#[test]
fn bad_values_are_refused_with_the_flag_and_the_value() {
    for (flag, value) in [
        ("--seeds", "9-3"),
        ("--seeds", "x"),
        ("--seeds", "0-0"),
        ("--workers", "0"),
        ("--workers", "many"),
    ] {
        let result = Options::parse(&args(&["--output", "o", flag, value]));

        assert_eq!(
            result,
            Err(OptionsError::BadValue {
                flag: flag.to_owned(),
                value: value.to_owned()
            }),
            "{flag} {value}"
        );
    }
}
