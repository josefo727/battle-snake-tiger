# Evidence: branch coverage of the search core (T029)

## Environment identity

- Repository commit under test: `e5a39df68a382621c3a7045932333e364cb49104` plus the T029 working tree (gate parser and script).
- Captured: 2026-09-18.
- Host: AMD Ryzen 7 5700X (8 cores / 16 threads), Linux 7.2.5, Docker 29.7.2.
- Toolchain inside the container: `rustc 1.100.0-nightly (330d31712 2026-09-17)` (`nightly-2026-09-18`), `cargo-llvm-cov 0.8.7`, report type `llvm.coverage.json.export` 3.1.0.
- Base image: `rust:1.98.1-slim-bookworm@sha256:ebd900bae66fd508b466cef82d64a83a5fb34682e4c8b2797a42908bddc95a57` (the same digest the sibling project pins); the derived image `tiger-engine-coverage:nightly-2026-09-18-llvm-cov-0.8.7` only adds the nightly toolchain and `cargo-llvm-cov`. The host toolchain was not touched.

## Procedure

```text
scripts/check-branch-coverage
  -> docker run --rm (repo and ../rules-core mounted read-only)
       cargo +nightly-2026-09-18 llvm-cov --branch --locked --json --output-path branch-coverage.json
  -> cargo test --test coverage_gate -- --ignored --exact the_real_engine_report_meets_the_branch_threshold
```

The instrumented run executes the whole suite of the workspace, including the process smoke test (which starts the instrumented binary over loopback TCP) and the gate self-tests. The gate (`engine/tests/coverage_gate.rs`) sums each module's branch tallies over its files, compares with an exact integer test (`covered * 100 >= count * 90`), and reports line and region percentages as supplemental figures only.

## Result: gate passed (exit 0)

| Module | Branches covered | Branch % | Line % (supplemental) | Region % (supplemental) |
|--------|------------------|----------|-----------------------|--------------------------|
| `engine/src/arena/` | 64 / 66 | 96.97 | 97.41 | 96.32 |
| `engine/src/valuation/` | 31 / 32 | 96.88 | 98.61 | 98.71 |
| `engine/src/lookahead/` | 38 / 38 | 100.00 | 98.92 | 98.89 |

Required floor: 90% branch coverage per module. Margin: 6.9, 6.9 and 10.0 points.

Per file (branches covered / total): arena: `duel.rs` 30/32, `cellset.rs` 8/8, `ingest.rs` 2/2, `serpent.rs` 24/24, `heading.rs` 0/0. Valuation: `mod.rs` 1/2, `dominion.rs` 14/14, `enclosure.rs` 12/12, `finish.rs` 2/2, `leverage.rs` 2/2, `fill.rs` 0/0, `sustenance.rs` 0/0. Lookahead: `minimax.rs` 18/18, `allowance.rs` 6/6, `deepening.rs` 6/6, `ordering.rs` 8/8, `ledger.rs` 0/0.

## The three uncovered branch arms

| Site | Arm | Why it is not covered |
|------|-----|-----------------------|
| `arena/duel.rs:162` (`loses_head_to_head`) | the false side of `!reports[side].off_board`, the first operand of the head-to-head condition | The head-to-head test is only asked for snakes that stayed on the board, so the operand was true in all 197,219 evaluations; the guard is defensive. |
| `arena/duel.rs:232` (`segments_without_tail`) | `serpent.length() == 1` | Reachable positions have length of at least 3 (a length-1 snake eating is a state the reused resolver treats as a self collision and real games never produce); the arm is defensive. |
| `valuation/mod.rs:100` (`Ledger::record`) | the capacity-overflow guard's false side | The ledger holds eight entries and five assessors are registered; the guard backs a `debug_assert` and cannot overflow today. |

## How to read the numbers

- LLVM branch coverage for Rust instruments conditional expressions (`if`, `&&`, `||`, `while`, match guards), not every `match` arm; a small branch count per module is normal (66, 32 and 38 here). The percentages say the conditionals are exercised on both sides, not that every path is.
- Generic code (the searcher is generic over the assessor set and the move order) has several instantiations; the module figures merge them.
- `gateway/` and `main.rs` are outside the gate by design. For information: `gateway/beacon.rs` 6/6, `gateway/http.rs` 6/6, `verdict/service.rs` 4/4, `verdict/report.rs` 2/2, `main.rs` 0/2 (the binary is exercised by the process tests as a separate process, which this run does not merge).

## Reproduce

`scripts/check-branch-coverage` (first run builds the derived image, several minutes; later runs reuse it). The report is written to `target/coverage/branch-coverage.json` (git-ignored).
