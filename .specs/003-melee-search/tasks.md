# Tasks - 003-melee-search

## Legend

Same legend as `001-duel-search`: `R` Red, `G` Green, `F` Refactor, statuses `open | in_progress | closed | skipped`.

Ordering: the N-snake kernel and its differential equivalence to the reused resolver (T001-T003) precede the valuation (T004-T008); the search (T009-T012) consumes both; the application and transport (T013-T015) consume the search; evidence on the finished engine (T016-T018) precedes the sparring extension (T019-T023) and the placement benchmark (T024), because the benchmark measures a gated engine.

## T001 - Represent a melee position with up to four seats

```yaml
id: T001
status: closed
commits: { reconstructed: c12dd32d03d56dc27999f6f6b54d284b4c43c8c3 }
source-commits: { red: ff1f1b3, green: 0b76907, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles II, IX, X
```

**Definition of Done**

- `Seat` names one of four positions; our seat is always seat 0.
- `MeleeBoard::try_new(&[Serpent], pellets)` accepts two to four serpents and rejects overlaps, a pellet under a serpent and a vigor outside `1..=100`, like the duel board.
- `alive_count`, `is_alive`, `seats()` (living seats in order), `serpent(seat)`, `pellets`, `ply` and `occupied` (every living body cell) are available.
- `as_duel()` converts a two-seat position (us plus one survivor) into a `DuelBoard` with our seat first, keeping ply and pellets.

**R - Red:** Add the scaffold with `try_new` accepting everything and `occupied` empty, and tests expecting the invariants and the occupied cells of three serpents; record the wrong results.

**G - Green:** Implement the checks, the seat accessors and the duel conversion.

**F - Refactor:** Skipped - the invariant check was shared with the duel board in the Green itself (one helper, both boards), so no duplication is left to remove.

**Files**

- `engine/src/arena/melee.rs`
- `engine/src/arena/mod.rs`
- `engine/src/arena/duel.rs`
- `engine/tests/melee_board.rs`

## T002 - Resolve movement and feeding for every seat at once

```yaml
id: T002
status: closed
commits: { reconstructed: 949619e56fb9fa2194ca5bb306fa32425b8e1688 }
source-commits: { red: 4ccaf7a, green: 1eb500b, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles II, IV, X
```

**Definition of Done**

- `ingest_melee(&TurnState)` converts three- and four-snake states with our snake at seat 0 (in the others' original order) and refuses any other count.
- `MeleeBoard::advance(&[Heading; 4])` moves every living seat from the same starting board, drops one vigor, feeds every head on a pellet (several heads on one pellet all eat) and removes eaten pellets.
- On ordinary (non-colliding) joint moves of three and four snakes the bodies, health and pellets equal the reused resolver's.
- The test support generates legal three- and four-snake states.

**R - Red:** Add `ingest_melee` and an `advance` that returns the board unchanged, with a differential test on ordinary joint moves; record the unmoved bodies.

**G - Green:** Implement ingest, the movement and feeding phases over the seats.

**F - Refactor:** Skipped - the duel kernel's move_serpent and MoveReport were reused in the Green (made crate-visible), so nothing was copied.

**Files**

- `engine/src/arena/melee.rs`
- `engine/src/arena/ingest.rs`
- `engine/src/arena/duel.rs`
- `engine/tests/support/mod.rs`
- `engine/tests/melee_differential.rs`

## T003 - Eliminate seats by the N-snake rules and report the outcome

```yaml
id: T003
status: closed
commits: { reconstructed: 05b88f5cb9703e9e462e29f11c9149a92a3f3eee }
source-commits: { red: f93e286, green: ff1b4ed, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles II, IV, XII
```

**Definition of Done**

- A seat is eliminated by zero vigor, leaving the board, entering any living body's post-move segments (own included, vacating tails excepted) or losing a head-to-head: on a cell with several heads only a strictly longest head survives.
- Eliminated seats are removed from the next board; a seat that left the board still blocks with its moved body this turn.
- `advance` returns `MeleeOutcome`: `Continues(board)` while we and at least one opponent live, `WeAlone` when we are the last, `WeDown { rivals_left }` when we die.
- A property over generated three- and four-snake positions (all 64 joint moves for three, a sample for four) agrees with the reused resolver on who survives, bodies, health and pellets.

**R - Red:** Add the outcome type with no eliminations and examples (three heads on one cell, equal longest heads, a leaver blocking) plus the property; record the survivors that should have died.

**G - Green:** Implement the elimination phase over all seats and the outcome.

**F - Refactor:** Skipped - the head-to-head judgement is already its own function reading as the rule (any other head there at least as long).

**Files**

- `engine/src/arena/melee.rs`
- `engine/tests/melee_differential.rs`
- `engine/tests/melee_board.rs`

Note: the Red's dead-seat example was a head-to-head, not a neck collision; the scenario was corrected in the Green commit.


## T004 - Make the valuation pipeline generic over the board and add attrition

```yaml
id: T004
status: closed
commits: { reconstructed: 12490ef5d5c2ed742bd7edafcf4c98af6dc490c4 }
source-commits: { red: 5c68fe1, green: 8173cf3, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1 and 5
contract-ref: n/a
constitution-ref: Articles II, X
```

**Definition of Done**

- `Assessor` declares the board type it assesses; the pipeline, ledger and worst-case machinery serve both `DuelBoard` and `MeleeBoard` without duplication; every duel test passes unchanged.
- `Attrition` (melee) is `4 - living seats`: one point per seat already eliminated, `MAX_RAW` 3.

**R - Red:** Add the generic trait with the duel assessors adapted, `Attrition` returning 0, and a test expecting 1 with three seats alive; record the zero.

**G - Green:** Implement the count.

**F - Refactor:** Skipped - the worst-case bound moved to its own Bounded trait in the Red, which is what let the tuple impls drop the board parameter; nothing else to tidy.

**Files**

- `engine/src/valuation/mod.rs`
- `engine/src/valuation/melee/mod.rs`
- `engine/src/valuation/melee/attrition.rs`
- `engine/tests/valuation_melee.rs`

## T005 - Multi-source territory for up to four seats

```yaml
id: T005
status: closed
commits: { reconstructed: 9aa018963dc8ccd207b8cb301b2cd7cddc83f962 }
source-commits: { red: 93dcd67, green: 6dbe8e1, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles II, X, XII
```

**Definition of Done**

- `Territory` partitions the cells among the living seats by a time-aware fill from every head (bodies free as their tails move, stacked copies delay), a cell reached the same turn by several seats going to the strictly longest or to nobody.
- The survey also gives each seat's distance to the nearest pellet it owns.
- Raw score: our cells minus the most cells any opponent owns; `MAX_RAW` 121.
- An independent oracle (per-cell earliest arrival by Dijkstra over the freeing schedule) agrees on generated positions.

**R - Red:** Add `Territory` returning 0 and a hand-built example expecting a known partition; record the zero.

**G - Green:** Implement the N-source fill on top of `Fill`.

**F - Refactor:** Skipped - the duel Dominion's settle step is a two-set special case of the per-seat contested union here; folding both would cost the duel fill a loop for no gain.

**Files**

- `engine/src/valuation/melee/territory.rs`
- `engine/src/valuation/melee/mod.rs`
- `engine/tests/valuation_melee_territory.rs`

Note: the Red's hand example had the wrong food distance (1 instead of 4); corrected in the Green commit, the oracle property was right from the start.


## T006 - Length standing and head danger among several heads

```yaml
id: T006
status: closed
commits: { reconstructed: 71a976620c116f415081cb4e2115c2d662c59e17 }
source-commits: { red: 735e1a1, green: 7805b21, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles II, X
```

**Definition of Done**

- `Standing`: our length minus the longest living opponent's; `MAX_RAW` 120.
- `HeadDanger`: over the cells we can enter next turn, minus one for each cell an equal-or-longer opponent head can also enter, plus one for each cell only shorter heads can enter; `MAX_RAW` 4.

**R - Red:** Scaffolds returning 0 and examples (two longer heads beside us; one shorter) expecting the signed counts; record the zeros.

**G - Green:** Implement both.

**F - Refactor:** Skipped - the enterable-cells helper is three lines over N seats; sharing it with the two-serpent HeadPressure would make the duel term depend on the melee module.

**Files**

- `engine/src/valuation/melee/standing.rs`
- `engine/src/valuation/melee/mod.rs`
- `engine/tests/valuation_melee.rs`

## T007 - Terminal scores by placement

```yaml
id: T007
status: closed
commits: { reconstructed: 49478e960931d335488063bfbc5752c1570386b7 }
source-commits: { red: cbac800, green: 3e91e3d, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1 and 3
contract-ref: n/a
constitution-ref: Articles II, X
```

**Definition of Done**

- `MeleeFinish` scores `WeAlone` at ply `p` as `win - ply_penalty * p`, `WeDown { rivals_left: k }` as `-win + ply_penalty * p + placement_step * (3 - k)` and a simultaneous wipe-out (`k == 0`) as `draw_score`; plies beyond `MAX_PLY` score as `MAX_PLY`.
- `sentinel` exceeds every score; every terminal magnitude is at least `finite_limit`; dying later or behind fewer survivors always scores higher.
- `MeleeWeights` carries these and the assessor weights, with a reasoned default profile.

**R - Red:** Add the finish returning 0 and a table expecting the ordering (later death beats earlier, second place beats third); record the zeros.

**G - Green:** Implement the formulas and bounds.

**F - Refactor:** Skipped - no smell detected; the finish is one match over the outcome.

**Files**

- `engine/src/valuation/melee/finish.rs`
- `engine/src/valuation/melee/weights.rs`
- `engine/src/valuation/melee/mod.rs`
- `engine/tests/valuation_melee_finish.rs`

## T008 - Hunger and the composed melee valuation with the duel hand-off

```yaml
id: T008
status: closed
commits: { reconstructed: 23c106886e3e34cc2f7d6a9a267976e1c5a8cbdd }
source-commits: { red: 4c4d6d8, green: 0b1f9d8, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles II, X, XII
```

**Definition of Done**

- `Hunger`: minus our pressure (the duel formula over our distance to the nearest owned pellet); `MAX_RAW` 60.
- `MeleeValuation::standard()` scores a position with three or four seats by the melee pipeline and one with two seats by the duel pipeline plus the attrition term, so the two scales meet.
- Its worst-case magnitude is below the melee finish's finite limit; scores are deterministic.

**R - Red:** `Hunger` returning 0 and the composed valuation returning 0, with tests expecting the pressure of a starving seat and the duel score for two seats; record the zeros.

**G - Green:** Implement both.

**F - Refactor:** Skipped - the pressure formula stays in sustenance and is reused as a public function; nothing duplicated.

Note: the Red's starving example put the pellet where a rival owned it (pressure 60, not 32); the example was corrected after the Green in a follow-up test commit, the implementation unchanged.

**Files**

- `engine/src/valuation/melee/hunger.rs`
- `engine/src/valuation/melee/mod.rs`
- `engine/src/valuation/sustenance.rs`
- `engine/tests/valuation_melee.rs`

## T009 - Move ordering keyed by seat

```yaml
id: T009
status: closed
commits: { reconstructed: e1b7e5ca9754e584ab7cff0ea98d0519c893a8b2 }
source-commits: { red: 892a9db, green: 25efe8f, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1 and 5
contract-ref: n/a
constitution-ref: Articles II, X
```

**Definition of Done**

- `HeadingOrder` arranges headings for a `Seat`; `Side` converts to a seat (`Us` 0, `Them` 1) so every duel call site and test is unchanged.
- `LearnedOrder` keeps killers and history for four seats; the previous best leads only seat 0 at ply 0.

**R - Red:** A test recording a cutoff for seat 3 and expecting it first for seat 3 and not for seat 2; record the failure.

**G - Green:** Widen the tables and the trait.

**F - Refactor:** Skipped - no smell detected; the trait takes any seat-convertible key and the duel call sites are untouched.

**Files**

- `engine/src/lookahead/ordering.rs`
- `engine/src/arena/melee.rs`
- `engine/tests/lookahead_ordering.rs`

## T010 - Fixed-depth paranoid alpha-beta over the seats

```yaml
id: T010
status: closed
commits: { reconstructed: pending }
source-commits: { red: e2c391f, green: 8adf307, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1 and 3
contract-ref: n/a
constitution-ref: Articles II, VIII, X, XII
```

**Definition of Done**

- `MeleeSearcher::search_fixed(board, depth)` returns the best heading and its exact paranoid minimax value: a maximizing layer over our headings, then one minimizing layer per living opponent in seat order, then `advance`; ties go to the earliest heading.
- Finished games score by `MeleeFinish`, leaves by `MeleeValuation`; nodes are counted per joint move.
- An exhaustive paranoid reference in the tests agrees at depths 1 and 2 on generated three- and four-snake positions; a position with a surviving heading never returns a certain loss when one exists at depth 1.

**R - Red:** Scaffold returning the first heading with score 0 and the reference comparison; record the mismatch.

**G - Green:** Implement the recursion with fail-soft windows.

**F - Refactor:** Skipped - Window and Interrupted were shared with the duel searcher from the Red (crate-visible); the three layers mirror the duel's shape.

**Files**

- `engine/src/lookahead/paranoid.rs`
- `engine/src/lookahead/minimax.rs`
- `engine/src/lookahead/mod.rs`
- `engine/tests/lookahead_paranoid.rs`

Note: the Red's node-count assertion expected the exhaustive 256 joint moves; alpha-beta visits 83 with the same answer, so the Green corrected the assertion to a bound.


## T011 - Opponent self-preservation pruning below the root

```yaml
id: T011
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles II, X
```

**Definition of Done**

- Below ply 0 an opponent tries only headings that stay on the board and avoid every cell a living body still holds after this turn's tail releases; with no such heading it keeps all four.
- The searched value equals the reference restricted the same way; the node count drops on a crowded position.

**R - Red:** A test expecting fewer nodes than the unpruned count on a fixed position and equality with the filtered reference; record the equal count.

**G - Green:** Implement the filter in the opponent layers.

**F - Refactor:** Skipped unless the filter duplicates `HeadDanger`'s enterable cells.

**Files**

- `engine/src/lookahead/paranoid.rs`
- `engine/tests/lookahead_paranoid.rs`

## T012 - Iterative deepening shared by both searchers

```yaml
id: T012
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 3
contract-ref: n/a
constitution-ref: Articles II, X, XI
```

**Definition of Done**

- `search_until` on the melee searcher gives up at the stop signal with nothing partial.
- The deepening driver works on any `IterativeSearch`; `deepen` for duels keeps its signature and tests.
- With a fake clock the melee driver reports the deepest completed depth, discards an interrupted iteration, stops early on a decisive score and reports nothing when no depth fits.

**R - Red:** A melee deepening test with a clock that expires mid-iteration expecting the previous depth; record the scaffold's depth 0.

**G - Green:** Extract the driver behind the trait and implement the melee stop.

**F - Refactor:** Skipped - the extraction is the Green.

**Files**

- `engine/src/lookahead/deepening.rs`
- `engine/src/lookahead/paranoid.rs`
- `engine/tests/lookahead_deepening.rs`

## T013 - Route three and four snakes to the melee search

```yaml
id: T013
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1 and 5
contract-ref: n/a
constitution-ref: Articles II, IX
```

**Definition of Done**

- `Route::MeleeSearch { state, board }` for supported three- and four-snake requests, with our seat first; duels, one snake and unsupported requests route as before.

**R - Red:** Change the route test to expect the melee route; record the safety fallback.

**G - Green:** Implement the selection.

**F - Refactor:** Skipped - no smell expected.

**Files**

- `engine/src/verdict/route.rs`
- `engine/tests/verdict_route.rs`

## T014 - Decide melee moves under the allowance

```yaml
id: T014
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1, 3 and 4
contract-ref: contracts/decision-diagnostic.schema.json
constitution-ref: Articles II, XI
```

**Definition of Done**

- `VerdictService` answers three- and four-snake requests with the melee search's heading at the completed depth, `EnginePath::MeleeSearch`, depth and nodes, and a terminal-win reason for a proven win.
- When no depth completes the reused one-turn safety decision answers with `budget_exhausted_before_first_depth`.
- Determinism with the manual clock; every request shape still returns a platform move.

**R - Red:** Tests expecting `MeleeSearch` with depth at least 1 for a four-snake request; record the safety fallback path.

**G - Green:** Implement the melee branch.

**F - Refactor:** Share the search-to-report step with the duel branch.

**Files**

- `engine/src/verdict/service.rs`
- `engine/src/verdict/report.rs`
- `engine/tests/verdict_service.rs`

## T015 - Diagnostic contract 2.1.0 and the transport

```yaml
id: T015
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 4 and 5
contract-ref: contracts/decision-diagnostic.schema.json
constitution-ref: Articles II, VII
```

**Definition of Done**

- `contracts/decision-diagnostic.schema.json` 2.1.0 adds `melee_search` to `engine_path` and is otherwise 2.0.0.
- Events validate against 2.1.0 for every path; the real router answers four-snake requests with a move and a `melee_search` event; `GET /`, `/start`, `/end` unchanged.

**R - Red:** The beacon test expecting `melee_search` and `2.1.0`; record the `2.0.0` mismatch.

**G - Green:** Bump the beacon and add the contract file.

**F - Refactor:** Skipped - no smell expected.

**Files**

- `.specs/003-melee-search/contracts/decision-diagnostic.schema.json`
- `engine/src/gateway/beacon.rs`
- `engine/tests/support/schema.rs`
- `engine/tests/gateway_beacon.rs`
- `engine/tests/gateway_http.rs`

## T016 - Profile the melee search

```yaml
id: T016
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1; plan.md §Risks
contract-ref: n/a
constitution-ref: Articles XIV
```

**Definition of Done**

- The profile harness reports throughput and the depth reached in 370 ms on generated four-snake positions, and the median depth is at least 3 (one full extra turn for every snake is depth 2).
- `evidence/profile-melee.md` records the numbers and the decision on further pruning.

**R - Red:** The harness expecting a median depth of at least 3 with the unpruned searcher stub; record the observed depth.

**G - Green:** Wire the real searcher and record the evidence.

**F - Refactor:** Skipped.

**Files**

- `engine/tests/profile_melee.rs`
- `scripts/run-profile`
- `.specs/003-melee-search/evidence/profile-melee.md`

## T017 - Loopback latency with four-snake requests

```yaml
id: T017
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 2
contract-ref: n/a
constitution-ref: Articles XI, XIV
```

**Definition of Done**

- The latency harness sends four-snake requests at concurrency 16 and the p99 stays at most `timeout - 120 ms`; `evidence/latency-melee.md` records it.

**R - Red:** The harness with the four-snake body expecting the bound; record the observed p99 if it fails, else the harness is the Green with the evidence.

**G - Green:** Record the evidence.

**F - Refactor:** Skipped.

**Files**

- `engine/tests/latency.rs`
- `.specs/003-melee-search/evidence/latency-melee.md`

## T018 - Branch coverage of the new modules

```yaml
id: T018
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: plan.md §Test strategy
contract-ref: n/a
constitution-ref: Article XII
```

**Definition of Done**

- The container coverage run reports arena, valuation and lookahead (melee modules included) at or above 90% branch coverage; `evidence/coverage.md` records it.

**R - Red:** Run the gate; if a module is below, add the missing tests (that is the Red).

**G - Green:** Record the evidence.

**F - Refactor:** Skipped.

**Files**

- `.specs/003-melee-search/evidence/coverage.md`

## T019 - Placements from a four-snake transcript

```yaml
id: T019
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 6
contract-ref: contracts/melee-report.schema.json
constitution-ref: Articles II, XIV
```

**Definition of Done**

- `parse_melee(text, names)` returns each snake's placement: the winner (or the survivors of a draw) first, then by the turn each snake last appears on the board, eliminated in the same turn sharing the average place; and the game length.
- Errors as the duel parser: empty, not JSON, missing result, an unknown name.

**R - Red:** Scaffold returning placement 1 for everyone and a four-snake transcript expecting `[1, 3.5, 2, 3.5]`; record the ones.

**G - Green:** Implement it.

**F - Refactor:** Share the line walk with `parse_transcript`.

**Files**

- `sparring/src/transcript.rs`
- `sparring/tests/transcript.rs`

## T020 - Play a four-snake bout through the official CLI

```yaml
id: T020
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 6
contract-ref: contracts/opponent-roster.yaml
constitution-ref: Articles II, XIV
```

**Definition of Done**

- `Bout { seats: Vec<Contestant>, seed }` and `MeleeRunner::play_bout` run `battlesnake play` with one `--name/--url` pair per seat and read the placements.
- The arguments for four seats are exact; the fake CLI test proves the transcript is read from the seat names.

**R - Red:** The argument test expecting four pairs; record the two.

**G - Green:** Implement it.

**F - Refactor:** Build the duel arguments from the bout arguments.

**Files**

- `sparring/src/runner.rs`
- `sparring/tests/runner.rs`

## T021 - A stop command for launched servers and Sansón in the roster

```yaml
id: T021
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 6
contract-ref: contracts/opponent-roster.yaml
constitution-ref: Articles II, VIII
```

**Definition of Done**

- A roster launch may carry a `stop` command (with `{port}`); the process launcher runs it when the server is dropped.
- `contracts/opponent-roster.yaml` 1.2.0 documents `stop`, Sansón (`docker run` of the local image) and the four-snake board.
- `reference/roster-melee.json` is written by the provisioning script with Sansón and two Flood seats.

**R - Red:** A roster test expecting the parsed stop command; record the missing field.

**G - Green:** Parse it and run it on drop.

**F - Refactor:** Skipped - no smell expected.

**Files**

- `sparring/src/roster.rs`
- `sparring/src/launcher.rs`
- `sparring/tests/roster.rs`
- `sparring/tests/launcher.rs`
- `scripts/provision-opponents`

## T022 - The placement benchmark and its report

```yaml
id: T022
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 6
contract-ref: contracts/melee-report.schema.json
constitution-ref: Articles II, XIV
```

**Definition of Done**

- `run_melee_benchmark` launches the challenger, the baseline and every opponent, plays each seed twice (challenger in seat one, then the baseline in that seat, the opponents in the other seats) and writes a `melee-report` 1.0.0: per game the seed, turns and each snake's placement; per snake role the mean placement.
- The verdict: the challenger's mean is strictly below the baseline's and not above Sansón's mean in the challenger's games.

**R - Red:** A fake-runner test expecting the means and the verdict; record the scaffold's empty report.

**G - Green:** Implement it.

**F - Refactor:** Share the launch step with the duel benchmark.

**Files**

- `sparring/src/benchmark.rs`
- `sparring/src/ledger.rs`
- `.specs/003-melee-search/contracts/melee-report.schema.json`
- `sparring/tests/benchmark.rs`
- `sparring/tests/ledger.rs`

## T023 - `spar --melee` and the sparring script

```yaml
id: T023
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 6
contract-ref: n/a
constitution-ref: Article XIV
```

**Definition of Done**

- `spar --melee` runs the placement benchmark with the roster's opponents as seats and exits 0 when the verdict holds, 2 otherwise.
- `scripts/run-sparring` passes the flag through and the melee roster is the default for it.

**R - Red:** An options test expecting `melee: true`; record the missing flag.

**G - Green:** Implement it.

**F - Refactor:** Skipped.

**Files**

- `sparring/src/options.rs`
- `sparring/src/main.rs`
- `sparring/tests/options.rs`
- `scripts/run-sparring`

## T024 - Run the placement benchmark

```yaml
id: T024
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 6
contract-ref: contracts/melee-report.schema.json
constitution-ref: Articles XIV
```

**Definition of Done**

- Seeds 1 to 30, the engine and the baseline each against Sansón and two Flood; `evidence/placement.md` records the means, the per-game placements and whether criterion 6 is met.
- If it is not met, weight iterations are recorded there and the last accepted profile is the one committed.

**R - Red:** The benchmark run is the observation; nothing to scaffold.

**G - Green:** The evidence file and any weight change it justifies.

**F - Refactor:** Skipped.

**Files**

- `.specs/003-melee-search/evidence/placement.md`
- `.specs/003-melee-search/evidence/melee-report.json`
- `engine/src/valuation/melee/weights.rs`
