# Tasks - 001-duel-search

## Legend

- `T{NNN}` - task id, unique within the feature, zero-padded.
- `[P]` - safe to execute in parallel with other `[P]` tasks (disjoint files, no shared mutable state).
- `R` - Red beat: a compiling scaffold plus a test whose assertion fails for a semantic reason.
- `G` - Green beat: the minimal implementation.
- `F` - Refactor beat: or "skipped" with a reason.
- `status` - `open | in_progress | closed | skipped`.

Ordering: the kernel and its differential equivalence to the reused resolver (T003-T010) precede every valuation and search task; valuation (T011-T018) precedes search (T019-T022); the application and transport layers (T023-T027) consume search; quality gates and evidence (T028-T033) precede the sparring subsystem (T034-T038) because sparring measures a finished, gated engine; packaging (T039) is last.


## T001 - Establish the workspace and engine version sentinel

```yaml
id: T001
status: closed
commits: { reconstructed: d6b39f30518001241089a09b5d6d1749c7824cff }
source-commits: { red: 0664bcf, green: 783ce0c, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 6
contract-ref: n/a
constitution-ref: Articles I, II, XIII
```

**Definition of Done**

- Cargo workspace with one member `engine` (package `tiger-engine`), Rust 1.98.1 edition 2024 pinned by `rust-toolchain.toml`.
- Runtime and dev dependencies use the exact pins from `plan.md` §Stack decision; `rules-core` is a path dependency and `Cargo.lock` is committed.
- `tiger_engine::ENGINE_VERSION` is exactly `0.1.0`.
- `cargo test --locked` runs the sentinel test.

**R - Red:** Add the manifests, toolchain file, lockfile, a compiling `ENGINE_VERSION` scaffold equal to the empty string, and a sentinel test expecting `0.1.0`; record the semantic mismatch.

**G - Green:** Set the constant to `0.1.0` and record the passing locked run.

**F - Refactor:** Skipped - no smell detected; the version has one definition and one observable contract assertion, and the manifest pins each dependency once.

**Files**

- `Cargo.toml`
- `rust-toolchain.toml`
- `engine/Cargo.toml`
- `engine/src/lib.rs`
- `engine/tests/sentinel.rs`

## T002 - Pin the rules-core dependency behind a facade

```yaml
id: T002
status: closed
commits: { reconstructed: 540bde5156ab9ec8563b21de62fc79c8673ccba6 }
source-commits: { red: d1eaf40, green: a6a96c5, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 5
contract-ref: contracts/rules-core-dependency.rs
constitution-ref: Articles IV, X
```

**Definition of Done**

- `engine/src/rules_core.rs` is the only module naming `rules_core` paths; every item in the contract compiles through it.
- `supported_snake_count` returns the snake count for a supported scope and `None` for an unsupported one.
- A real two-snake request with `ruleset.version` `v1.2.3` and one with `cli` both classify as supported with two snakes.
- A wrong-version request classifies as unsupported.

**R - Red:** Add the facade with `supported_snake_count` returning `None` for every input and a contract test expecting `Some(2)` for a duel request; record the wrong result.

**G - Green:** Implement the helper over the reused `classify` and re-export the contract items.

**F - Refactor:** Skipped - no smell detected; the request builders already live in the shared test support module from the first commit, and the facade names each sibling path exactly once.

**Files**

- `engine/src/rules_core.rs`
- `engine/src/lib.rs`
- `engine/tests/rules_core_contract.rs`
- `engine/tests/support/mod.rs`

## T003 - Represent cells and 121-bit cell sets

```yaml
id: T003
status: closed
commits: { reconstructed: c57eeeb3c1fdf9f549c87d1b1b1a7ea5287314e3 }
source-commits: { red: 37e0230, green: aa78b90, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles IX, X, XII
```

**Definition of Done**

- `Cell` is valid only for indices 0..=120 and maps to and from (x, y) with index = y * 11 + x.
- `CellSet` never holds bits above 120 after any operation (union, intersection, difference, complement, shifts).
- Directional neighbour expansion respects the left and right edges (a corner has two neighbours, an interior cell four).
- Iterating a set yields cells in ascending index order and `len` equals the number of yielded cells.

**R - Red:** Add compiling `Cell` and `CellSet` scaffolds whose `expand` returns the set unchanged, plus an edge-wrap example test; record the wrong neighbour count.

**G - Green:** Implement masked shifts and the operations; add property tests for the 121-bit invariant and neighbour symmetry.

**F - Refactor:** Skipped - no smell detected; the edge masks were already named constants built by one const fn, and each operation is a single expression over the same private word.

**Files**

- `engine/src/arena/mod.rs`
- `engine/src/arena/cellset.rs`
- `engine/src/lib.rs`
- `engine/tests/cellset.rs`

## T004 - Model headings and bounded steps

```yaml
id: T004
status: closed
commits: { reconstructed: de2e599350357f7945e09b77712593010810defe }
source-commits: { red: 8107e6d, green: 764b8ed, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles IX, X
```

**Definition of Done**

- `Heading` has four values in a fixed order (north, east, south, west) with a stable index.
- `step(cell)` returns `None` when the step leaves the board and the neighbouring `Cell` otherwise.
- Stepping in a heading and then in its opposite returns the original cell whenever both steps are on the board.

**R - Red:** Add a `Heading` scaffold whose `step` always returns `None`; record the failing in-board step example.

**G - Green:** Implement `step` over the cell arithmetic and add the round-trip property.

**F - Refactor:** Skipped - no smell detected; the delta table has a single definition and step and the CellSet shifts are cross-checked by a property test.

**Files**

- `engine/src/arena/heading.rs`
- `engine/src/arena/mod.rs`
- `engine/tests/heading.rs`

## T005 - Model serpents with ring-buffer bodies

```yaml
id: T005
status: closed
commits: { reconstructed: 0f3bd0257d2bb270f8f97a0050ac18e9baaae49d }
source-commits: { red: 406bf75, green: ad31fd5, refactor: 53bd517 }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles X, XII
```

**Definition of Done**

- A serpent's ring, length, and `cells` mirror agree after every operation (property).
- `advance_head` adds a cell and `release_tail` removes the oldest without allocating.
- A growth stack (tail repeated once) keeps the shared cell occupied until the second copy is released.
- Length never exceeds 121.

**R - Red:** Add a `Serpent` scaffold whose `release_tail` does nothing; record the occupancy mismatch after a move.

**G - Green:** Implement the ring operations and the mirror invariant with property tests.

**F - Refactor:** Extracted slot_behind_head so the ring's slot arithmetic has one definition, shared by segment lookup and tail stacking.

**Files**

- `engine/src/arena/serpent.rs`
- `engine/src/arena/mod.rs`
- `engine/tests/serpent.rs`

## T006 - Build duel boards and ingest turn states

```yaml
id: T006
status: closed
commits: { reconstructed: 81413c9f3539c161f4c99fef61a100776c41c259 }
source-commits: { red: 8134c7d, green: bdcdfaa, refactor: 43b67b1 }
spec-ref: spec.md §Acceptance criteria 1 and 5
contract-ref: contracts/rules-core-dependency.rs
constitution-ref: Articles IX, X
```

**Definition of Done**

- `DuelBoard::try_new` rejects overlapping serpents, food under a serpent, and out-of-range vigor with typed errors.
- `ingest` builds a `DuelBoard` from a reused `TurnState` with our snake first, preserving ordered bodies, health, and food.
- `ingest` fails with `IngestError::NotADuel` for one, three, or four snakes.
- Round trip: the board's serpent cells equal the `TurnState` occupancy.

**R - Red:** Add `ingest` returning `NotADuel` for every input and a test expecting a two-snake state to convert; record the wrong error.

**G - Green:** Implement construction, invariants, and the conversion.

**F - Refactor:** Hand-written Debug for Serpent (body and vigor instead of the raw ring), prompted by unreadable Red failure output; conversion helpers were already separate from the invariant checks.

**Files**

- `engine/src/arena/duel.rs`
- `engine/src/arena/ingest.rs`
- `engine/src/arena/mod.rs`
- `engine/tests/ingest.rs`

Note: the reused TurnState only holds two to four snakes, so a one-snake input is unrepresentable; NotADuel is exercised for three and four snakes.


## T007 - Advance ordinary movement, health, and tail release

```yaml
id: T007
status: closed
commits: { reconstructed: 49c76bd93617d1d52f5bd4b190ab71f2f0be8221 }
source-commits: { red: d7ee649, green: 71aa5dd, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: contracts/rules-core-dependency.rs
constitution-ref: Articles II, XII
```

**Definition of Done**

- For non-eating, non-colliding joint moves, `advance` equals the reused `resolve_turn` on bodies, health, and survival (differential example suite).
- Health drops by one per turn and the old tail is released before any collision test.
- Both snakes move simultaneously from the same immutable starting board.

**R - Red:** Add `advance` as a scaffold returning the unchanged board plus a differential example against `resolve_turn`; record the body mismatch.

**G - Green:** Implement head movement, tail release, and health decrement.

**F - Refactor:** Skipped - move_serpent was extracted as the movement phase in the Green commit itself, so later phases can reuse it without a separate refactor.

**Files**

- `engine/src/arena/duel.rs`
- `engine/tests/advance_differential.rs`
- `engine/tests/support/mod.rs`

Note: Green also touched engine/src/arena/serpent.rs (Serpent::lose_vigor), beyond the declared files.


## T008 - Advance food consumption and growth

```yaml
id: T008
status: closed
commits: { reconstructed: 813dec255df29e0fa309ceb89ce8d9672bc73e39 }
source-commits: { red: 6601f22, green: 0786f03, refactor: f3c1532 }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: contracts/rules-core-dependency.rs
constitution-ref: Articles II, XII
```

**Definition of Done**

- Eating restores health to 100, removes the food, and duplicates the current post-move tail.
- A later non-eating move releases one copy of a stacked tail while the other stays occupied.
- Agreement with `resolve_turn` on the food and growth examples, including two serpents eating the same cell.

**R - Red:** Add the growth examples to the differential suite with `advance` ignoring food; record the length mismatch.

**G - Green:** Implement the food phase after ordinary movement.

**F - Refactor:** Extracted movement_phase and feeding_phase so advance reads in the official resolution order.

**Files**

- `engine/src/arena/duel.rs`
- `engine/tests/advance_differential.rs`

Note: Green also touched engine/src/arena/serpent.rs (Serpent::eat) and the test support module (state_after and the lockstep runner) beyond the declared files.


## T009 - Advance eliminations

```yaml
id: T009
status: closed
commits: { reconstructed: c80302d96bf78ffb55ef339b21e6fbe91aae4688 }
source-commits: { red: 145b893, green: 0452471, refactor: 580be77 }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: contracts/rules-core-dependency.rs
constitution-ref: Articles II, XII
```

**Definition of Done**

- Starvation, leaving the board, and self or body collisions eliminate exactly as `resolve_turn` does.
- Eliminations are decided on the post-move snapshot for both serpents simultaneously.
- `Advance::Over` carries `WeOnly`, `TheyOnly`, or `BothDown` matching the reference survivors.

**R - Red:** Add elimination examples with `advance` never eliminating; record the missing `Over` result.

**G - Green:** Implement the elimination phase and the verdict mapping.

**F - Refactor:** Extracted is_eliminated so the phase reads as vigor, off-board, self-hit, or opposing segments, then a verdict mapping.

**Files**

- `engine/src/arena/duel.rs`
- `engine/tests/advance_differential.rs`

Note: the kernel keeps per-move reports (off_board, self_hit, segments) so an off-board serpent's body snapshot matches the reused resolver's (old head kept as a segment, tail released); no source file beyond the declared ones was touched.


## T010 - Advance head-to-head and prove kernel equivalence

```yaml
id: T010
status: closed
commits: { reconstructed: a4a19c9ed5379228ab4dd3dade7007fea2017115 }
source-commits: { red: 1e7ea89, green: fceffae, refactor: e04a2a8 }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: contracts/rules-core-dependency.rs
constitution-ref: Articles II, XII
```

**Definition of Done**

- Head-to-head: the longer serpent survives, equal lengths eliminate both, matching the reference.
- A proptest generates legal duel states and all 16 joint moves and asserts full agreement with `resolve_turn` on survival, bodies, health, and consumed food.
- The suite runs inside the normal `cargo test --locked`.

**R - Red:** Add head-to-head examples with `advance` treating them as no collision; record the survivor mismatch.

**G - Green:** Implement head-to-head resolution; enable the generated differential property.

**F - Refactor:** Moved the duel generators (state_spec, realize) into the shared test support module for reuse by the valuation and search property suites.

**Files**

- `engine/src/arena/duel.rs`
- `engine/tests/advance_differential.rs`
- `engine/tests/support/mod.rs`

Note: the generated property builds only reachable positions (length >= 3). The reused resolver treats a length-1 snake eating as a self collision, a state Standard games never produce, so the kernel is intentionally not required to match it there. The T008 same-pellet example was updated (it assumed the duel continues) and a two-pellet case added. Green also touched no file beyond the declared ones; soak: 60 independent property runs, 0 failures.


## T011 - Assemble the valuation pipeline and weight sheet

```yaml
id: T011
status: closed
commits: { reconstructed: a256f509044b728cb379a7ed4adf4d13dbb438cc }
source-commits: { red: efdf57b, green: 24dc82d, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles VI, X, XII
```

**Definition of Done**

- `Assessor` is one trait with one method; `ValuationPipeline` sums weighted contributions into an integer score and a `Ledger`.
- `WeightSheet` holds every coefficient by name and `DEFAULT_PROFILE` is one constant.
- With no assessors registered the score is 0 and the ledger is empty.

**R - Red:** Add a pipeline scaffold returning 1 and a test expecting 0 for the empty pipeline; record the wrong score.

**G - Green:** Implement the trait, ledger, weighted sum, and weight sheet.

**F - Refactor:** Skipped - no smell detected; each weight name appears once (WeightSheet) and the ledger only stores what a term reports.

**Files**

- `engine/src/valuation/mod.rs`
- `engine/src/valuation/weights.rs`
- `engine/src/lib.rs`
- `engine/tests/valuation_pipeline.rs`

Note: the assessor list is a static, nested-tuple AssessorSet built by ValuationPipeline::with (no vtable on the scoring path); Green touched no file beyond the declared ones.


## T012 - Score terminal positions

```yaml
id: T012
status: closed
commits: { reconstructed: eeee787d6f4b08c7725d3450a840a883fd1206ff }
source-commits: { red: c67ff12, green: a452497, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles XI, XII
```

**Definition of Done**

- `Finish` scores a forced win as a large positive value, a forced loss as its negation, and mutual elimination as a small draw score.
- Faster wins score higher than slower wins and slower losses score higher than faster ones (ply-distance term).
- Scores stay inside the documented bounds so search can use them as infinity sentinels.

**R - Red:** Add `Finish` returning 0 for every verdict; record the win-score failure.

**G - Green:** Implement terminal scoring with the ply-distance term.

**F - Refactor:** Skipped - no smell detected; MAX_PLY is a named constant beside its only use and every coefficient already lives in the WeightSheet.

**Files**

- `engine/src/valuation/finish.rs`
- `engine/src/valuation/mod.rs`
- `engine/tests/valuation_finish.rs`

Note: Finish scores terminal outcomes for search; it is not registered in the positional pipeline (see the T018 amendment).


## T013 - Compute static Voronoi territory

```yaml
id: T013
status: closed
commits: { reconstructed: 258587c4d7aa382d4fedb4d88256a48b4ecaec67 }
source-commits: { red: 64d3cd7, green: 808471d, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles VIII, XII
```

**Definition of Done**

- `Dominion` returns cells reached strictly first by each serpent using layered fills over `CellSet`.
- Cells reached in the same layer go to the longer serpent and to nobody on equal length.
- Swapping the two serpents negates the territory difference (property).
- Obstacles are all serpent cells (no release yet).

**R - Red:** Add `Dominion` counting zero cells for both sides; record the wrong territory on an open-board example.

**G - Green:** Implement the layered fill and contested-cell rule.

**F - Refactor:** Skipped - expand_layer was extracted as its own function in the Green commit, ready for the time-aware variant in T014.

**Files**

- `engine/src/valuation/dominion.rs`
- `engine/src/valuation/mod.rs`
- `engine/tests/valuation_dominion.rs`

Note: a first draft of the sealed-pocket test was wrong (the wall serpent's head sat next to the pocket); it was replaced before the Red commit by a corner cell sealed with body segments.


## T014 - Model tail release in territory

```yaml
id: T014
status: closed
commits: { reconstructed: cd5dd577319055dfdad8b10f9d198bede5d7c7ab }
source-commits: { red: 62155cb, green: d1b89be, refactor: 37fd9b6 }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles VIII, XII
```

**Definition of Done**

- Layer `t` treats the tail segments released within `t` turns as passable (segment `i` from the tail is free after `i + 1` turns).
- A serpent chasing its own tail owns more territory than the static model reports on the reference example.
- Growth stacks delay release by one turn per stacked segment.
- The fill terminates within `MAX_LAYERS` and never reads bits outside the board.

**R - Red:** Add a tail-chase example expecting the enlarged territory while `Dominion` still uses static obstacles; record the count mismatch.

**G - Green:** Compute the obstacle set per layer from segment release times.

**F - Refactor:** Folded the paired frontier/seen state of each serpent into a Fill type (advance and is_exhausted); release masks are looked up in O(1) per turn, so no precomputation was needed.

**Files**

- `engine/src/valuation/dominion.rs`
- `engine/tests/valuation_dominion.rs`

Note: Green added Serpent::cell_from_tail (engine/src/arena/serpent.rs) beyond the declared files, and the Red beat replaced the two T013 tests that encoded the static rule (serpent cells belong to nobody; a cell sealed in by body segments) because segments now free in time.


## T015 - Assess sustenance

```yaml
id: T015
status: closed
commits: { reconstructed: d7cd26c8e354ed79627c80cc82a9a267513a3f64 }
source-commits: { red: 2b78c5a, green: 1c115c6, refactor: bc942bc }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles VIII, XII
```

**Definition of Done**

- `Sustenance` compares health margin with the distance to the nearest food each serpent reaches first.
- Urgency grows as the margin shrinks and is zero when health comfortably exceeds the distance.
- Swapping the serpents negates the contribution.

**R - Red:** Add `Sustenance` returning 0 and a low-health example expecting a negative contribution; record the mismatch.

**G - Green:** Implement the margin and urgency computation from the dominion fill's food distances.

**F - Refactor:** Extracted settle() so ownership of newly reached cells (including the tie rule) is computed once and shared by the partition update and the pellet-distance tracking; the food-distance helper is shared with Dominion through Survey.

**Files**

- `engine/src/valuation/sustenance.rs`
- `engine/src/valuation/mod.rs`
- `engine/tests/valuation_sustenance.rs`

Note: the task's declared files did not include engine/src/valuation/dominion.rs, which Green and Refactor changed (Survey and the food-distance tracking). Follow-up for T018/T030: each assessor that calls Dominion::survey recomputes the fill, so the standard pipeline pays for it twice per leaf; measure first, then share one Survey per position if it matters.


## T016 - Assess leverage and head-to-head pressure

```yaml
id: T016
status: closed
commits: { reconstructed: ed7c78ae1b0ff27dc446cf7892076db2e9e88329 }
source-commits: { red: fb1cfba, green: c163ba3, refactor: 06b7d7e }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles VIII, XII
```

**Definition of Done**

- `Leverage` rewards a length advantage and head-to-head threat coverage while we are longer, and penalises the reverse.
- Equal lengths produce no head-to-head term.
- Swapping the serpents negates the contribution.

**R - Red:** Add `Leverage` returning 0 and a longer-serpent example expecting a positive value; record the mismatch.

**G - Green:** Implement the length and threat terms.

**F - Refactor:** Moved the 'cell vacated on turn n unless a stacked copy holds it' rule into Serpent::cell_released_on_turn and pointed Dominion at it, removing the duplicate.

**Files**

- `engine/src/valuation/leverage.rs`
- `engine/src/valuation/mod.rs`
- `engine/tests/valuation_leverage.rs`

Note: Leverage was built as two single-question assessors, LengthAdvantage and HeadPressure (module valuation::leverage), so each keeps its own WeightSheet coefficient and ledger line; the standard pipeline in T018 registers five terms (see the amendment). Green also touched engine/src/arena/serpent.rs beyond the declared files.


## T017 - Estimate survival in separated regions

```yaml
id: T017
status: closed
commits: { reconstructed: dce4ad994e9d4f5d13e1f9d4d9e6eae8f5c59f7c }
source-commits: { red: fabc9eb, green: a88049a, refactor: ec14f41 }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles VIII, XII
```

**Definition of Done**

- `Enclosure` activates only when the two serpents' reachable regions are disjoint.
- Each side's estimate uses region size, parity-adjusted cell count, and tail release.
- The contribution is our estimate minus theirs and is antisymmetric under side swap.
- When regions overlap the contribution is exactly zero.

**R - Red:** Add `Enclosure` returning 0 and a walled-off example expecting a positive contribution for the larger region; record the mismatch.

**G - Green:** Implement region detection and the survival estimate.

**F - Refactor:** Moved Fill and MAX_LAYERS into valuation/fill.rs so territory and enclosure share the layered fill from a neutral module instead of one reaching into the other.

**Files**

- `engine/src/valuation/enclosure.rs`
- `engine/src/valuation/mod.rs`
- `engine/tests/valuation_enclosure.rs`

Note: Enclosure treats walls as static for the activation test (cells reachable through unoccupied cells) but lets a serpent's own body cells free on schedule for its survival estimate, and caps the estimate at health plus 100 per pellet in its room; a probe showed 7307 of 20000 generated duels activate the term. Bodies do release in real play, so 'separated' is a short-horizon judgement; sparring will show whether the term helps. Green also touched engine/src/valuation/dominion.rs beyond the declared files.


## T018 - Compose the default valuation profile

```yaml
id: T018
status: closed
commits: { reconstructed: 78291ad87a2f9d600d29a314e86a4f50555c8b1c }
source-commits: { red: ab1c340, green: 7e15ee3, refactor: 7e1cec4 }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles VIII, XII, XIV
```

**Definition of Done**

- `ValuationPipeline::standard()` registers `Dominion`, `Sustenance`, `LengthAdvantage`, `HeadPressure`, and `Enclosure` with `DEFAULT_PROFILE` (`Finish` scores terminal outcomes for search and is not a positional assessor).
- Property: swapping the serpents negates the total score on generated positions.
- Property: the score is deterministic and stays inside the finite-score bounds for non-terminal positions.
- The ledger lists one entry per registered assessor (five).

**R - Red:** Add `standard()` returning the empty pipeline and a symmetry test over an open-board example; record the zero score.

**G - Green:** Register the assessors with the default weights.

**F - Refactor:** Removed the unused contested_pellet coefficient from the WeightSheet (no assessor consumed it); registration order is written once in StandardPipeline::with_profile.

**Files**

- `engine/src/valuation/mod.rs`
- `engine/src/valuation/weights.rs`
- `engine/tests/valuation_pipeline.rs`

Note: beyond the declared files, Green changed the assessor modules (each declares MAX_RAW; Sustenance clamps pressure at MAX_PRESSURE = 60) and tests/valuation_sustenance.rs. The clamp and the declared bounds were added because the default weights could otherwise exceed the finite limit (worst case is now 69,220 against 75,000), so the 'stays inside the finite bounds' criterion is arithmetic, not only tested.


## T019 - Bound search by a clock-backed allowance

```yaml
id: T019
status: closed
commits: { reconstructed: 68151f28906c4a2f43ed7ca04c14925033579dca }
source-commits: { red: 06ae23b, green: cc4b441, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 2 and 4
contract-ref: contracts/decision-diagnostic.schema.json
constitution-ref: Articles VII, XI, XIV
```

**Definition of Done**

- `SearchAllowance` derives its deadline from the reused `response_deadline` minus `SEARCH_TAIL_MARGIN`.
- `should_stop` polls the injected `Clock` only every `POLL_INTERVAL_NODES` visited nodes and on demand.
- `LookaheadReport` records completed depth, nodes explored, best heading, and principal score.

**R - Red:** Add an allowance scaffold whose `should_stop` is always false and a fake-clock test expecting a stop at the deadline; record the missed stop.

**G - Green:** Implement the polling and deadline arithmetic.

**F - Refactor:** Skipped - no smell detected; the polling counter lives in one place (should_stop delegates to poll_now at the interval), and the expiry flag is the only state shared between them.

**Files**

- `engine/src/lookahead/mod.rs`
- `engine/src/lookahead/allowance.rs`
- `engine/src/lookahead/ledger.rs`
- `engine/tests/lookahead_allowance.rs`

Note: the task's declared files did not include the test support additions engine/tests/support/clock.rs (ManualClock, which counts clock reads) and engine/tests/support/mod.rs; T022 reuses ManualClock. Iteration-start policy (the elapsed fraction) is intentionally not here: it belongs to T022.


## T020 - Search a fixed depth with alpha-beta

```yaml
id: T020
status: closed
commits: { reconstructed: 0114dec725da889aca7a44ee89aebc3873f1b718 }
source-commits: { red: 5f68e15, green: 5feccbd, refactor: 2cad65d }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles VIII, XI, XII
```

**Definition of Done**

- `Maximizer` iterates our four headings and `Minimizer` the opponent's four, applying both moves through `advance`.
- Property: the alpha-beta value equals an exhaustive minimax reference at depths 1 to 3 on generated positions.
- The chosen heading is never a self-inflicted certain loss when a surviving heading exists.
- Terminal outcomes use `Finish` and interior leaves use the standard pipeline.

**R - Red:** Add `search_fixed` returning the first heading with score 0 and an exhaustive-minimax comparison example; record the value mismatch.

**G - Green:** Implement fail-soft alpha-beta over the two layers.

**F - Refactor:** Extracted: the root loop and the interior maximizer shared one window update, so `best_heading` now serves both (root keeps the heading, interior plies take the value); a separate minimizer layer stays because its bound update is the mirror image and merging would need a sign flag.

**Files**

- `engine/src/lookahead/minimax.rs`
- `engine/src/lookahead/mod.rs`
- `engine/tests/lookahead_minimax.rs`

Note: the Green commit's first implementation had two maximizer copies; the shared step was extracted in the Refactor. Only the node count is exposed, one per `advance`; ordering (T021) and the clock (T022) are not part of this task. The 8 tests include a forced-win example, a pruning-count check and three properties (soaked with 60 extra random-seed runs).


## T021 - Order moves for early cutoffs

```yaml
id: T021
status: closed
commits: { reconstructed: 5fe171e7e9d376481668aa193b70c724759e0122 }
source-commits: { red: 7bcf16c, green: 7d9353b, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1
contract-ref: n/a
constitution-ref: Articles VIII, XI
```

**Definition of Done**

- Ordering places the previous iteration's best heading first, then killer headings for the ply, then history-ranked headings.
- Ties fall back to the fixed heading order so results are deterministic.
- On a fixed position suite ordered search visits strictly fewer nodes than unordered search at the same depth and returns the same value.

**R - Red:** Add an ordering scaffold returning the fixed order and a node-count comparison expecting fewer nodes; record equal counts.

**G - Green:** Implement previous-best, killer, and history ordering.

**F - Refactor:** Skipped - no smell detected; the history table is indexed in one place (note_cutoff writes it, arrange reads it) and Lineup is the only place that deduplicates.

**Files**

- `engine/src/lookahead/ordering.rs`
- `engine/src/lookahead/minimax.rs`
- `engine/tests/lookahead_ordering.rs`

Note: beyond the declared files, the seam is a `HeadingOrder` trait with `NaturalOrder` and `LearnedOrder`, and `Searcher` gained a second type parameter (default `LearnedOrder`) plus `with_order`; T022 reuses one `Searcher` across depths so the previous best and history carry over. The root now searches an earlier heading one point wider than the best so ties resolve by `Heading::ALL` regardless of order; without it, ordering would change the chosen heading among equals. Measured node savings (release, six-position suite): 4x at depth 4, 3.9x at depth 5, 6.3x at depth 6.


## T022 - Deepen iteratively within the allowance

```yaml
id: T022
status: closed
commits: { reconstructed: 37bb6882e6d1a42fa3353dd0ad4b11f7db6e46a6 }
source-commits: { red: a556ecd, green: ef7e126, refactor: 9b5290c }
spec-ref: spec.md §Acceptance criteria 2, 3, and 4
contract-ref: contracts/decision-diagnostic.schema.json
constitution-ref: Articles XI, XIV
```

**Definition of Done**

- The driver runs depths 1, 2, 3, ... and never starts a depth after `ITERATION_START_FRACTION` of the allowance has elapsed.
- An interrupted iteration is discarded and the report carries the last completed depth's heading.
- If no depth completes the report says so explicitly.
- Determinism: identical input and fake clock yield identical reports.

**R - Red:** Add a driver scaffold returning depth 0 always and a fake-clock test expecting depth 3 with a generous allowance; record the depth mismatch.

**G - Green:** Implement the iteration loop, discard policy, and report assembly.

**F - Refactor:** Extracted `may_start_iteration(left, span)`, a pure predicate beside `ITERATION_START_PERCENT`; the loop no longer carries the arithmetic.

**Files**

- `engine/src/lookahead/deepening.rs`
- `engine/src/lookahead/mod.rs`
- `engine/tests/lookahead_deepening.rs`

Note: the plan's ITERATION_START_FRACTION (40%) is named `ITERATION_START_PERCENT` (integer percent, so the comparison stays in integer microseconds). The span is fixed by the first clock read of the driver, not by request arrival, so the fraction is of the time actually available to the search. Beyond the declared files: `StopSignal`/`NeverStop` and `SearchAllowance::time_left` in allowance.rs, `search_until`/`is_decisive`/`nodes_visited` and an internal `Window` in minimax.rs, and a `ScriptedClock` test double. A decisive score (|score| >= finite limit) ends deepening early (glossary: full-tree solve); `DEPTH_CEILING` (64) is clamped with `min` and has no test of its own because no feasible position reaches it.


## T023 - Select the engine route

```yaml
id: T023
status: closed
commits: { reconstructed: pending }
source-commits: { red: 9c3edba, green: 197aac1, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 5
contract-ref: contracts/rules-core-dependency.rs
constitution-ref: Articles IX, X
```

**Definition of Done**

- Exactly two snakes in a supported scope routes to `DuelSearch`.
- Three or four snakes in a supported scope routes to `SafetyFallback`.
- Any unsupported scope routes to `UnsupportedFallback`.
- The selector is a pure function of the classified scope.

**R - Red:** Add `RouteSelector` returning `UnsupportedFallback` always and a duel example expecting `DuelSearch`; record the wrong route.

**G - Green:** Implement the selection over the facade.

**F - Refactor:** Skipped - the selector does not use the facade's `supported_snake_count`; it matches the scope once and lets `ingest` tell a duel from anything else, so no branch table is duplicated.

**Files**

- `engine/src/verdict/mod.rs`
- `engine/src/verdict/route.rs`
- `engine/src/lib.rs`
- `engine/tests/verdict_route.rs`

Note: `Route::DuelSearch` carries a `Box<DuelBoard>` built by the selector, so the service (T024) never converts the request twice and has no unreachable 'ingest failed after routing' branch. A probe showed the reused classifier already rejects every state the kernel would refuse (health 0/101, overlap, one-segment snake, food under a snake, unknown `you`); the mapping of an ingest failure to `SafetyFallback` therefore is defensive and shares the branch that serves 3-4 snakes. The facade helper `supported_snake_count` is now used only by the contract tests.


## T024 - Decide moves through the verdict service

```yaml
id: T024
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1, 2, 3, and 5
contract-ref: contracts/rules-core-dependency.rs
constitution-ref: Articles VII, X, XI
```

**Definition of Done**

- The duel path returns the search report's heading with `selection_reason` `search_completed_depth` or `search_terminal_win`.
- When no depth completes the service returns the reused one-turn safety decision and `budget_exhausted_before_first_depth`.
- Non-duel routes return exactly what the reused `decide_within_deadline` or `decide_unsupported` returns.
- Every returned heading is one of the four platform-valid directions.

**R - Red:** Add `VerdictService` always delegating to the safety fallback and a duel example expecting a search-derived heading and depth above zero; record the wrong reason.

**G - Green:** Implement the pipeline over the route, the search driver, and the fallbacks.

**F - Refactor:** Extract `VerdictReport` construction.

**Files**

- `engine/src/verdict/service.rs`
- `engine/src/verdict/mod.rs`
- `engine/tests/verdict_service.rs`
- `engine/tests/support/mod.rs`

## T025 - Emit schema-versioned decision diagnostics

```yaml
id: T025
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 4
contract-ref: contracts/decision-diagnostic.schema.json
constitution-ref: Articles VII, XIV
```

**Definition of Done**

- `DecisionBeacon` is the only diagnostics port and the tracing adapter emits exactly one `move_decision` event per decision.
- Serialized events validate against schema 2.0.0 for all three engine paths.
- The event carries `game_id` and `turn` and never a board, body, name, or shout.
- `search_depth` and `nodes_explored` equal the report's values.

**R - Red:** Add an event serializer scaffold emitting `search_depth` 0 and a test expecting the report's depth; record the mismatch.

**G - Green:** Implement the event mapping, the beacon port, and the tracing adapter.

**F - Refactor:** Skipped unless reason mapping is duplicated.

**Files**

- `engine/src/gateway/mod.rs`
- `engine/src/gateway/beacon.rs`
- `engine/src/lib.rs`
- `engine/tests/gateway_beacon.rs`
- `engine/tests/support/mod.rs`

## T026 - Serve the four Battlesnake routes

```yaml
id: T026
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 2, 6, and 7
contract-ref: contracts/openapi.yaml
constitution-ref: Articles IV, VII, IX
```

**Definition of Done**

- `GET /` returns HTTP 200 JSON exactly `{"apiversion":"1","author":"josefo727","color":"#00D5FF","head":"tiger-king","tail":"tiger-tail","version":"0.1.0"}`.
- `POST /start` and `POST /end` return HTTP 200 with `{}` for any syntactically valid request.
- `POST /move` returns HTTP 200 with exactly one `move` member; malformed JSON is 400, wrong or missing content type 415, bodies over 64 KiB 413.
- A duel, a three-snake game, and an unsupported game each produce a valid move through the real router and emit one diagnostic.

**R - Red:** Add a router scaffold whose `/` returns `{}` and a contract test expecting the exact identity body; record the body mismatch.

**G - Green:** Implement the routes, validation, and the composition root `build_service`.

**F - Refactor:** Unify the acknowledgement handlers.

**Files**

- `engine/src/gateway/http.rs`
- `engine/src/gateway/mod.rs`
- `engine/src/lib.rs`
- `engine/tests/gateway_http.rs`

## T027 - Run the server process

```yaml
id: T027
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 2 and 6
contract-ref: contracts/openapi.yaml
constitution-ref: Articles VII, XI
```

**Definition of Done**

- `Settings::from_lookup` reads `BIND_ADDR` (default `0.0.0.0`) and `PORT` (default `8080`) and rejects invalid values with actionable errors.
- `main.rs` installs the production `SystemClock`, the tracing JSON subscriber, and serves the router.
- A test starts the compiled binary on a free port and receives the exact identity body over loopback TCP.

**R - Red:** Add `Settings` returning port 0 and a test expecting the default 8080; record the wrong default.

**G - Green:** Implement settings and `main`; add the loopback smoke test.

**F - Refactor:** Skipped unless config parsing duplicates the sibling helper's shape.

**Files**

- `engine/src/gateway/settings.rs`
- `engine/src/main.rs`
- `engine/tests/gateway_settings.rs`
- `engine/tests/process_smoke.rs`

## T028 - Enforce static quality gates

```yaml
id: T028
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1 to 8
contract-ref: n/a
constitution-ref: Articles X, XIII
```

**Definition of Done**

- `deny.toml` sets explicit advisories, bans, licenses, and sources policy for the workspace.
- `scripts/verify` composes `cargo fmt --check`, locked tests, Clippy with warnings denied, the inward-dependency architecture check, and `cargo deny check`.
- The architecture check fails when `arena`, `valuation`, `lookahead`, or `verdict` imports `axum`, `tokio`, `tracing`, `std::fs`, `std::process`, or `crate::gateway`.
- A failing stage exits non-zero and names the stage.

**R - Red:** Add a scaffold `scripts/verify` that always passes and an architecture-check self-test expecting failure on a planted forbidden import; record the false pass.

**G - Green:** Implement the stages and the check; write `deny.toml`.

**F - Refactor:** Keep the scripts small and free of CI-provider assumptions.

**Files**

- `deny.toml`
- `scripts/verify`
- `scripts/check-architecture`
- `engine/tests/architecture_check.rs`

## T029 - Gate branch coverage in a container

```yaml
id: T029
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1 to 5
contract-ref: n/a
constitution-ref: Article XII
```

**Definition of Done**

- A parser fails when any of `arena`, `valuation`, or `lookahead` is below 90% branch coverage and accepts at or above 90%.
- Line and region coverage are reported as supplemental evidence only.
- `scripts/check-branch-coverage` runs the pinned nightly and `cargo-llvm-cov` inside a disposable container built from the pinned Rust image, leaving the host toolchain untouched.
- The real run's percentages are recorded in the evidence file.

**R - Red:** Add a parser scaffold that accepts every report and a below-threshold fixture case; record the incorrect acceptance.

**G - Green:** Implement the threshold check per module and the container script; run it for real.

**F - Refactor:** Keep the script small and fail-fast.

**Files**

- `engine/tests/coverage_gate.rs`
- `scripts/check-branch-coverage`
- `.specs/001-duel-search/evidence/coverage.md`

## T030 - Measure kernel throughput and position repeat rate

```yaml
id: T030
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 4
contract-ref: n/a
constitution-ref: Articles XI, XIV
```

**Definition of Done**

- An ignored release-mode harness records kernel `advance` throughput and the reused `resolve_turn` throughput on the same states.
- It searches a fixed suite of 200 generated midgame duels and records nodes/second, completed depth, and the fraction of visited positions that repeat within one search.
- The evidence file records exact command, environment identity, and whether the 15% transposition gate passes.

**R - Red:** Add a harness scaffold that reports zero repeats and an assertion that the suite size is 200; record the zero-sample mismatch before adding measurements.

**G - Green:** Implement the measurements and evidence writer; run it for real.

**F - Refactor:** Separate measurement from assertion.

**Files**

- `engine/tests/profile.rs`
- `scripts/run-profile`
- `.specs/001-duel-search/evidence/profile.md`

## T031 - Memoize positions with a transposition table

```yaml
id: T031
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1 and 4
contract-ref: n/a
constitution-ref: Articles VI, XI, XIV
```

**Definition of Done**

- Executes only if T030 records at least 15% repeats; otherwise the task closes as skipped with the measured rate.
- Zobrist keys come from a const splitmix64 sequence; the table stores depth, bound type, best heading, and a signature with depth-preferred replacement.
- A stored heading is re-validated as legal before use; terminal scores are re-based by ply distance.
- Property: search with the table returns the same value as without it at depths 1 to 3.
- Nodes explored at equal depth are lower on the fixed suite.

**R - Red:** Add table scaffolds that never store and a test expecting a hit on a transposed position; record the missed hit.

**G - Green:** Implement keys, entries, replacement, and integration into the search.

**F - Refactor:** Isolate probing and storing behind two functions.

**Files**

- `engine/src/lookahead/memo.rs`
- `engine/src/lookahead/minimax.rs`
- `engine/tests/lookahead_memo.rs`

## T032 - Measure deadline, latency, and depth on loopback

```yaml
id: T032
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 2 and 4
contract-ref: contracts/openapi.yaml
constitution-ref: Articles VII, XI, XII, XIV
```

**Definition of Done**

- The release-mode harness starts a fresh loopback server with the production router and bounded diagnostics.
- It performs 1,000 warmups followed by 20,000 requests at concurrency 16 on worst-case duels and repeats a sequential comparison.
- The report records p50, p95, p99, maximum latency, cutoff count, invalid-move count, completed-depth distribution, and nodes/second.
- p99 completes no later than `timeout - 120 ms`, with zero invalid moves and completed depth of at least 2 on every duel decision.
- Re-running produces a versioned evidence record with the exact command and environment identity.

**R - Red:** Add a loopback-sampler scaffold and the workload-count acceptance test; record its zero-sample mismatch before adding percentile and evidence assertions.

**G - Green:** Implement the sampler, percentile calculation, and report command; run it for real.

**F - Refactor:** Separate measurement from assertion.

**Files**

- `engine/tests/latency.rs`
- `scripts/run-latency`
- `.specs/001-duel-search/evidence/latency.md`

## T033 - Drive the engine with the official rules CLI

```yaml
id: T033
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1 and 5
contract-ref: n/a
constitution-ref: Articles IV, V, IX
```

**Definition of Done**

- `scripts/fetch-rules-oracle` downloads and digest-verifies the official v1.2.3 CLI without committing it.
- An ignored suite plays seeded duel games of the compiled engine against the sibling one-turn baseline and completes without protocol errors.
- At least 95% of the engine's recorded decisions in those games report `engine_path` `duel_search` (guards against a repeat of the sibling's local-CLI classification blind spot).
- A three-snake CLI game completes using the safety fallback.

**R - Red:** Add the suite scaffold with a recorder that counts zero duel decisions and an expectation of at least 95%; record the mismatch.

**G - Green:** Implement fetching, process launching, and decision counting; run it for real.

**F - Refactor:** Share process helpers with the latency harness.

**Files**

- `scripts/fetch-rules-oracle`
- `engine/tests/cli_endtoend.rs`
- `engine/tests/support/mod.rs`
- `.gitignore`

## T034 - Compute win rates with Wilson intervals

```yaml
id: T034
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 8
contract-ref: contracts/sparring-report.schema.json
constitution-ref: Articles XIV
```

**Definition of Done**

- A new workspace member `sparring` (package `tiger-sparring`) compiles.
- `win_rate` counts draws as neither wins nor losses in the numerator and reports them separately.
- The 95% Wilson interval matches reference values for (0/30), (15/30), and (30/30) within 1e-9.
- Empty samples are rejected with a typed error.

**R - Red:** Add a statistics scaffold returning a zero interval and a reference-value test for 15/30; record the mismatch.

**G - Green:** Implement win rate and the Wilson interval.

**F - Refactor:** Skipped unless formulas are duplicated.

**Files**

- `Cargo.toml`
- `sparring/Cargo.toml`
- `sparring/src/lib.rs`
- `sparring/src/statistics.rs`
- `sparring/tests/statistics.rs`

## T035 - Run and parse official-CLI games

```yaml
id: T035
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 8
contract-ref: contracts/opponent-roster.yaml
constitution-ref: Articles VII, XIV
```

**Definition of Done**

- `SparringRunner` is a port with an official-CLI adapter that runs one seeded duel between two local URLs and returns winner, turns, and seed.
- Result parsing handles a win, a loss, a draw, and an unparsable transcript (typed error).
- The runner checks each opponent's liveness (`GET /`) before the first game and fails fast when one is unreachable.

**R - Red:** Add a parser scaffold returning `Draw` for every transcript and a win-transcript example; record the wrong result.

**G - Green:** Implement parsing, the adapter, and the liveness check.

**F - Refactor:** Separate process spawning from parsing.

**Files**

- `sparring/src/runner.rs`
- `sparring/src/transcript.rs`
- `sparring/src/lib.rs`
- `sparring/tests/runner.rs`

## T036 - Persist versioned sparring reports

```yaml
id: T036
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 8
contract-ref: contracts/sparring-report.schema.json
constitution-ref: Articles VII, XIV
```

**Definition of Done**

- `ReportSink` is a port with a JSON file adapter that writes a report validating against schema 1.0.0.
- The report records engine commit, environment identity, opponents, seeds, wins, losses, draws, win rate, and Wilson interval.
- Writing refuses to overwrite an existing report without an explicit flag.

**R - Red:** Add a sink scaffold that writes an empty object and a schema-conformance test; record the missing required fields.

**G - Green:** Implement the report assembly and file adapter.

**F - Refactor:** Skipped unless assembly duplicates the statistics module.

**Files**

- `sparring/src/ledger.rs`
- `sparring/src/lib.rs`
- `sparring/tests/ledger.rs`

## T037 - Provision opponents and run the first sparring benchmark

```yaml
id: T037
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 8
contract-ref: contracts/opponent-roster.yaml
constitution-ref: Articles VIII, XIV
```

**Definition of Done**

- `scripts/provision-opponents` builds Shapeshifter from the existing local checkout with default features and clones and builds Flood inside this workspace's ignored reference directory, recording each opponent's launch command in the roster.
- `spar` runs seeds 1 to 30 per opponent for the challenger and for the sibling baseline and writes a schema-valid report.
- The first real report is committed as evidence with exact environment and engine commit.

**R - Red:** Add a `spar` scaffold that reports zero games and an assertion that 30 games were played per matchup; record the zero-sample mismatch.

**G - Green:** Implement orchestration and provisioning; run the benchmark for real.

**F - Refactor:** Separate roster loading from orchestration.

**Files**

- `sparring/src/main.rs`
- `sparring/src/roster.rs`
- `scripts/provision-opponents`
- `scripts/run-sparring`
- `.specs/001-duel-search/evidence/sparring.md`

## T038 - Satisfy the sparring acceptance bar by measured iteration

```yaml
id: T038
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 8
contract-ref: contracts/sparring-report.schema.json
constitution-ref: Articles VIII, XIV
```

**Definition of Done**

- Each weight or ordering change is a separate commit with the before and after sparring numbers in its message and in the iteration log.
- A change that measures worse is reverted and logged as a dead end.
- The final report shows a win rate strictly greater than the sibling baseline's against both Shapeshifter and Flood over seeds 1 to 30.
- Win rates against both opponents and the trend across iterations are recorded whether or not they approach parity.

**R - Red:** Add the iteration-log acceptance check that fails while any matchup's challenger win rate does not exceed the baseline's; record the failing report.

**G - Green:** Iterate the default profile and search parameters until the check passes.

**F - Refactor:** Remove any experiment scaffolding not adopted.

**Files**

- `.specs/001-duel-search/evidence/sparring.md`
- `engine/src/valuation/weights.rs`
- `engine/tests/sparring_gate.rs`

## T039 - Package the engine in a digest-pinned image

```yaml
id: T039
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 2 and 6
contract-ref: n/a
constitution-ref: Articles XIII
```

**Definition of Done**

- A multi-stage Dockerfile pins the same builder and runtime image digests as the sibling and builds the release binary with `--locked`.
- The image runs as a non-root user and works with a read-only root filesystem.
- `scripts/container-smoke` builds the image, runs it, and checks all four routes including the exact identity body.
- The image contains neither an opponent nor the rules CLI.

**R - Red:** Add a smoke script scaffold that asserts an empty identity body and record its mismatch against the real routes.

**G - Green:** Add the Dockerfile and smoke script; run them for real.

**F - Refactor:** Skipped unless the smoke script duplicates a helper.

**Files**

- `Dockerfile`
- `.dockerignore`
- `scripts/container-smoke`

## Coverage matrix

| Acceptance criterion | Owning tasks |
|---|---|
| 1 Multi-turn evaluation for duels | T003-T022, T024 |
| 2 Valid move within `timeout - 120 ms` | T019, T022, T024, T026, T027, T032, T039 |
| 3 Best completed depth on cutoff | T022, T024 |
| 4 Depth and nodes reported | T019, T022, T025, T030, T032 |
| 5 Non-duel and unsupported fallback | T002, T023, T024, T026, T033 |
| 6 Exact `GET /` identity | T001, T026, T027, T039 |
| 7 `/start` and `/end` acknowledgements | T026 |
| 8 Sparring benchmark vs. Shapeshifter and Flood | T034-T038 |

| Constitution article | Enforced by |
|---|---|
| VII boundaries (Clock, diagnostics, SparringRunner, ReportSink) | T019, T025, T035, T036 |
| VIII independent authorship and divergence | T013, T037, plan.md design-diff table, PROVENANCE.md |
| X inward dependencies | T028 |
| XI deadline safety | T019, T022, T032 |
| XII 90% branch coverage and properties | T010, T018, T020, T029 |
| XIII pinned dependencies | T001, T028, T039 |
| XIV measurable strength | T030, T032, T037, T038 |

## Ordering and parallelism audit

All tasks execute sequentially: kernel tasks share `duel.rs` and its differential suite; valuation tasks share `valuation/mod.rs`; search tasks share `lookahead/minimax.rs`; the sparring tasks share the `sparring` crate. No `[P]` flag is used because no pair of tasks has disjoint files and independent contracts. T031 may close as `skipped` if T030's measured repeat rate is below 15%.

## Amendments

| Date | Change | Reason |
|------|--------|--------|
| 2026-09-15 | T018 registers four positional assessors, not five; `Finish` is applied by search to terminal outcomes. | Terminal verdicts come from `advance` as `Over(Verdict)`, not from a position, so `Finish::score(verdict, ply)` cannot implement the position-taking `Assessor` trait (found in T012). |
| 2026-09-16 | T016's `Leverage` is two assessors, `LengthAdvantage` and `HeadPressure`; T018 registers five terms. | Each of the two questions has its own `WeightSheet` coefficient (`length_advantage`, `head_pressure`), and the pipeline multiplies one raw value by one weight, so separate assessors keep the weights independent and the ledger informative (found while designing T016). |
| 2026-09-16 | T023's `Route` borrows the data its engine needs (`DuelSearch { state, board }`, `SafetyFallback(&TurnState)`, `UnsupportedFallback(&FallbackContext)`). | Found while designing T024: with unit variants the service would have to re-match the scope and keep an unreachable arm; borrowing removes both. Behaviour of the selector is unchanged. |
