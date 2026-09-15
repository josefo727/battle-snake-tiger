use std::time::{SystemTime, UNIX_EPOCH};

use proptest::prelude::*;
use tiger_engine::gateway::calendar::{Calendar, CivilDate, SystemCalendar};

fn date(year: i32, month: u8, day: u8) -> CivilDate {
    CivilDate { year, month, day }
}

#[test]
fn known_instants_map_to_their_utc_day() {
    let table = [
        (0, date(1970, 1, 1)),
        (86_399, date(1970, 1, 1)),
        (86_400, date(1970, 1, 2)),
        (-1, date(1969, 12, 31)),
        (951_782_400, date(2000, 2, 29)),
        (951_868_799, date(2000, 2, 29)),
        (951_868_800, date(2000, 3, 1)),
        (1_709_164_800, date(2024, 2, 29)),
        (1_789_776_000, date(2026, 9, 19)),
        (4_107_542_400, date(2100, 3, 1)),
    ];

    for (seconds, expected) in table {
        assert_eq!(CivilDate::from_unix_seconds(seconds), expected, "{seconds}");
    }
}

/// The next calendar day, by the rules a child would count them.
fn tomorrow(d: CivilDate) -> CivilDate {
    let leap = (d.year % 4 == 0 && d.year % 100 != 0) || d.year % 400 == 0;
    let length = match d.month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if leap => 29,
        _ => 28,
    };
    match (d.day == length, d.month == 12) {
        (false, _) => date(d.year, d.month, d.day + 1),
        (true, false) => date(d.year, d.month + 1, 1),
        (true, true) => date(d.year + 1, 1, 1),
    }
}

#[test]
fn every_day_from_1970_to_2100_follows_the_previous_one() {
    let mut expected = date(1970, 1, 1);
    let mut seconds = 0i64;

    while expected.year < 2100 {
        assert_eq!(CivilDate::from_unix_seconds(seconds), expected, "{seconds}");
        assert_eq!(CivilDate::from_unix_seconds(seconds + 86_399), expected);
        expected = tomorrow(expected);
        seconds += 86_400;
    }
}

#[test]
fn a_date_is_written_as_year_month_day_with_zero_padding() {
    assert_eq!(date(2026, 9, 19).to_string(), "2026-09-19");
    assert_eq!(date(1970, 1, 1).to_string(), "1970-01-01");
    assert_eq!(date(2100, 12, 31).to_string(), "2100-12-31");
}

#[test]
fn a_written_date_reads_back_and_bad_text_does_not() {
    assert_eq!(CivilDate::parse("2026-09-19"), Some(date(2026, 9, 19)));
    assert_eq!(CivilDate::parse("2000-02-29"), Some(date(2000, 2, 29)));
    for bad in [
        "",
        "2026-9-19",
        "2026-09-9",
        "26-09-19",
        "2026/09/19",
        "2026-13-01",
        "2026-00-10",
        "2026-09-00",
        "2026-09-31",
        "2026-02-29",
        "1900-02-29",
        "2026-09-19x",
        " 2026-09-19",
        "2026-09-19\n",
        "abcd-ef-gh",
        "+026-09-19",
    ] {
        assert_eq!(CivilDate::parse(bad), None, "{bad:?}");
    }
}

proptest! {
    #[test]
    fn every_instant_round_trips_through_its_text(seconds in -2_000_000_000i64..4_200_000_000) {
        let day = CivilDate::from_unix_seconds(seconds);

        prop_assert_eq!(CivilDate::parse(&day.to_string()), Some(day));
    }

    #[test]
    fn dates_order_like_the_instants(a in 0i64..4_000_000_000, b in 0i64..4_000_000_000) {
        let (da, db) = (CivilDate::from_unix_seconds(a), CivilDate::from_unix_seconds(b));

        prop_assert_eq!(a.div_euclid(86_400).cmp(&b.div_euclid(86_400)), da.cmp(&db));
    }
}

#[test]
fn the_system_calendar_reports_todays_utc_day() {
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let reported = SystemCalendar.today();

    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!(
        reported == CivilDate::from_unix_seconds(before)
            || reported == CivilDate::from_unix_seconds(after)
    );
    assert!(reported.year >= 2026);
}
