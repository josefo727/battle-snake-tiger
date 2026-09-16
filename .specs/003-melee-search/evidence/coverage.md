# Evidence: branch coverage of the search core with the melee modules (003 T018)

## Environment identity

- Repository commit under test: `abb90b8` (the melee engine complete; the stop-command test rewritten for the container, which has no python).
- Captured: 2026-09-19, 08:30 to 08:45 UTC.
- Host: AMD Ryzen 7 5700X (8 cores / 16 threads), Linux 7.2.5, Docker 29.7.2.
- Toolchain inside the container: `nightly-2026-09-18`, `cargo-llvm-cov 0.8.7`, the same derived image as `001-duel-search/evidence/coverage.md` (`tiger-engine-coverage:nightly-2026-09-18-llvm-cov-0.8.7`). The host toolchain was not touched.

## Procedure

`scripts/check-branch-coverage`, unchanged: the whole workspace suite runs instrumented in the container (process smoke included), then the gate test sums each module's branch tallies over its files and applies the exact integer test `covered * 100 >= count * 90`.

## Result: gate passed (exit 0)

| Module | Branch % | Line % (supplemental) | Region % (supplemental) |
|--------|----------|-----------------------|--------------------------|
| `engine/src/arena/` | 95.00 (95 / 100) | 97.91 | 96.86 |
| `engine/src/valuation/` | 98.15 (53 / 54) | 98.94 | 99.01 |
| `engine/src/lookahead/` | 98.48 (65 / 66) | 99.38 | 99.39 |

Required floor: 90% branch coverage per module. Margin: 5.0, 8.2 and 8.5 points.

Per file (branches covered / total), the new modules first: arena `melee.rs` 27/30, `ingest.rs` 6/6 (was 2/2), `duel.rs` 30/32 (unchanged); valuation `melee/territory.rs` 18/18, `melee/standing.rs` 2/2, `melee/mod.rs` 2/2, `melee/attrition.rs`, `melee/finish.rs`, `melee/hunger.rs` 0/0 (no branch points; their arithmetic is covered by lines), `mod.rs` 1/2 (unchanged); lookahead `paranoid.rs` 27/28, `deepening.rs` 6/6, `minimax.rs` 18/18, `ordering.rs` 8/8, `allowance.rs` 6/6.

## The new uncovered branch arms

| Site | Arm | Why it is not covered |
|------|-----|-----------------------|
| `arena/melee.rs` (`as_duel`) | the failure side of `DuelBoard::try_new(...).expect(...)` | A valid melee position with two living seats is always a valid duel; the arm is the `expect`'s message path. |
| `arena/melee.rs` (`loses_head_to_head`) | the true side of `reports[seat].off_board` at the top of the head-to-head check | A seat that left the board is eliminated before the head-to-head is asked in every generated position; the guard is defensive, like its duel twin (`duel.rs:162`). |
| `arena/melee.rs` (`try_new`) | one side of the seat-count range check | `2..=MAX_SEATS` is one condition with two arms in the instrumentation; the one-serpent and five-serpent examples cover the refusal, the other arm is the accepted count's second bound. |
| `lookahead/paranoid.rs` (`replies_of`) | `safe.is_empty()` true: an opponent with no self-preserving heading keeps all four | The boxed-opponent example reaches it at depth 2 only when the search does not prune that seat away first; the instrumented run happened to cut it off. The behaviour is covered by the reference comparison in `lookahead_paranoid.rs`. |

The three arms left uncovered by feature 001 are unchanged (`001-duel-search/evidence/coverage.md`).
