mod support;

use proptest::prelude::*;
use tiger_engine::arena::cellset::Cell;
use tiger_engine::arena::duel::Side;
use tiger_engine::rules_core::{Scope, classify};
use tiger_engine::verdict::route::{Route, RouteSelector};

use support::request_with;

fn scope(version: &str, snakes: usize, timeout: i64, you: usize) -> Scope {
    classify(&request_with(version, snakes, timeout, you, &[]))
}

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("on the board")
}

#[test]
fn two_snakes_in_a_supported_scope_go_to_the_duel_search() {
    let classified = scope("v1.2.3", 2, 500, 0);
    let route = RouteSelector::select(&classified);

    assert!(matches!(route, Route::DuelSearch { .. }), "{route:?}");
}

#[test]
fn the_local_cli_identifier_is_also_a_duel() {
    let classified = scope("cli", 2, 500, 0);
    let route = RouteSelector::select(&classified);

    assert!(matches!(route, Route::DuelSearch { .. }), "{route:?}");
}

#[test]
fn the_duel_route_carries_the_board_with_our_snake_first() {
    // Snake 1 starts at (2, 2); when it is "you" it must be the Us side.
    let classified = scope("v1.2.3", 2, 500, 1);
    let route = RouteSelector::select(&classified);

    let Route::DuelSearch { board, .. } = route else {
        panic!("expected the duel route, got {route:?}");
    };
    assert_eq!(board.serpent(Side::Us).head(), cell(2, 2));
    assert_eq!(board.serpent(Side::Them).head(), cell(5, 5));
}

#[test]
fn three_or_four_snakes_in_a_supported_scope_use_the_safety_fallback() {
    for snakes in [3, 4] {
        let classified = scope("v1.2.3", snakes, 500, 0);
        let route = RouteSelector::select(&classified);

        assert!(
            matches!(route, Route::SafetyFallback(_)),
            "{snakes}: {route:?}"
        );
    }
}

#[test]
fn an_unsupported_scope_uses_the_unsupported_fallback() {
    let cases = [
        ("an unsupported ruleset version", scope("v9.9.9", 2, 500, 0)),
        (
            "a timeout at the response reserve",
            scope("v1.2.3", 2, 120, 0),
        ),
        ("a lone snake", scope("v1.2.3", 1, 500, 0)),
        ("no snakes", scope("v1.2.3", 0, 500, 0)),
    ];

    for (label, scope) in cases {
        let route = RouteSelector::select(&scope);

        assert!(
            matches!(route, Route::UnsupportedFallback(_)),
            "{label}: {route:?}"
        );
    }
}

#[test]
fn selection_is_pure_the_same_scope_always_gets_the_same_kind_of_route() {
    let scope = scope("v1.2.3", 2, 500, 0);

    let first = RouteSelector::select(&scope);
    let second = RouteSelector::select(&scope);

    let (Route::DuelSearch { board: a, .. }, Route::DuelSearch { board: b, .. }) = (first, second)
    else {
        panic!("both selections must be the duel route");
    };
    let body = |board: &tiger_engine::arena::duel::DuelBoard, side| {
        board.serpent(side).body().collect::<Vec<_>>()
    };
    assert_eq!(body(&a, Side::Us), body(&b, Side::Us));
    assert_eq!(body(&a, Side::Them), body(&b, Side::Them));
    assert_eq!(a.pellets(), b.pellets());
}

#[test]
fn every_route_borrows_the_data_of_the_scope_it_was_selected_from() {
    let duel = scope("v1.2.3", 2, 500, 0);
    let melee = scope("v1.2.3", 3, 500, 0);
    let unsupported = scope("v9.9.9", 2, 500, 0);

    let Route::DuelSearch { state, .. } = RouteSelector::select(&duel) else {
        panic!("expected the duel route");
    };
    let Route::SafetyFallback(melee_state) = RouteSelector::select(&melee) else {
        panic!("expected the safety route");
    };
    let Route::UnsupportedFallback(context) = RouteSelector::select(&unsupported) else {
        panic!("expected the unsupported route");
    };

    let (Scope::Supported(duel_state), Scope::Supported(expected_melee)) = (&duel, &melee) else {
        panic!("both scopes are supported");
    };
    let Scope::Unsupported(expected_context) = &unsupported else {
        panic!("the third scope is unsupported");
    };
    assert!(std::ptr::eq(state, duel_state));
    assert!(std::ptr::eq(melee_state, expected_melee));
    assert!(std::ptr::eq(context, expected_context));
    assert_eq!(melee_state.snakes().len(), 3);
}

/// The specification's routing table, written independently of the selector.
fn expected(version: &str, snakes: usize, timeout: i64) -> &'static str {
    let certified = version == "v1.2.3" || version == "cli";
    if !certified || timeout <= 120 || snakes < 2 {
        "unsupported"
    } else if snakes == 2 {
        "duel"
    } else {
        "safety"
    }
}

fn kind(route: &Route) -> &'static str {
    match route {
        Route::DuelSearch { .. } => "duel",
        Route::SafetyFallback(_) => "safety",
        Route::UnsupportedFallback(_) => "unsupported",
    }
}

proptest! {
    #[test]
    fn the_route_follows_the_specified_table_for_every_request_shape(
        version in prop_oneof![Just("v1.2.3"), Just("cli"), Just("v9.9.9")],
        snakes in 0usize..=4,
        timeout in 100i64..=600,
        you in 0usize..=1,
    ) {
        let you = you.min(snakes.saturating_sub(1));

        let classified = scope(version, snakes, timeout, you);
    let route = RouteSelector::select(&classified);

        prop_assert_eq!(kind(&route), expected(version, snakes, timeout));
    }
}
