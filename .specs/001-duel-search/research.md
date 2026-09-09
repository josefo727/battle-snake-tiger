# Research - 001-duel-search

## Research method

Context7 tools were not available in the active tool catalog on 2026-09-18, so the fallback sources below are primary documentation: the official Battlesnake docs, the Chess Programming Wiki pages on classical game-tree search, peer-reviewed and community write-ups of two-player territory games, and the versioned sibling project's own recorded evidence. Only publicly documented techniques and README-level descriptions were consulted. No Shapeshifter source, tests, fixtures, identifiers, constants, or module layout informed any decision below (see `PROVENANCE.md` for one incidental-exposure disclosure). `snork`'s README was read for its documented methodology only, not its source.

## Battlesnake API and Standard rules@captured-2026-09-18

- **captured:** 2026-09-18
- **source:**
  - https://docs.battlesnake.com/rules
  - https://docs.battlesnake.com/api/example-move
  - https://docs.battlesnake.com/api/objects/ruleset
  - `../rules-core/.specs/001-safe-move-api/research.md` (same normative sources, already verified against the official CLI)
- **why consulted:** Fix the exact turn-resolution order the compact search kernel must reproduce.

### Relevant API shape

```text
Turn resolution order (docs.battlesnake.com/rules):
 1. Move: new head added in the chosen direction, tail removed, health -1.
 2. Food: on a food cell health resets to maximum, an extra segment is
    placed on top of the current tail, the food is removed.
 3. Food placement (not modelled inside search).
 4. Eliminations: health <= 0, out of bounds, self collision, collision
    with another snake, head-to-head loss (longer survives, equal length
    eliminates both).
```

### Gotchas, rate limits, versioning

- Local `battlesnake play` reports `ruleset.version == "cli"` (already handled by the sibling's ADR 0006); this project inherits that classification through the reused predicate.
- Food spawning is random and cannot be predicted; search treats spawned food as unknown and only sees food present at the root plus food consumed along a line.

### Decision impact

- Ties to `plan.md` §Data model and ADR 0002: the compact kernel reproduces exactly this order and is differentially tested against the sibling resolver.

---

## Alpha-beta pruning@Chess Programming Wiki

- **captured:** 2026-09-18
- **source:** https://www.chessprogramming.org/Alpha-Beta
- **why consulted:** Choose the core pruning algorithm for the duel search.

### Relevant API shape

```text
alpha-beta keeps [alpha, beta] score bounds; fail-soft returns scores outside
the window (retains more information than fail-hard).
Best-case leaf count with branching b and depth n: b^ceil(n/2) + b^floor(n/2) - 1
(vs b^n for plain minimax); depth 6 example: ~128,000 vs ~4.1 billion.
```

### Gotchas, rate limits, versioning

- The gain is entirely a function of move ordering: searching the best move first eliminates the most nodes; the worst ordering degenerates to plain minimax.

### Decision impact

- Ties to ADR 0003: alpha-beta over the serialized simultaneous-move game, fail-soft, with ordering as a first-class component rather than an afterthought.

---

## Iterative deepening@Chess Programming Wiki

- **captured:** 2026-09-18
- **source:** https://www.chessprogramming.org/Iterative_Deepening
- **why consulted:** Time-bounded search that always holds a legal best move (spec criterion 3).

### Relevant API shape

```text
search depth 1, 2, 3, ... until the allowance is spent; keep the last
COMPLETED iteration's best move; use its principal variation / hash move
first in the next iteration.
```

### Gotchas, rate limits, versioning

- The redundant work of shallower iterations is small next to the ordering benefit; an interrupted iteration must be discarded (or only trusted when its first move was fully searched).

### Decision impact

- Ties to spec criteria 3 and 4 and Article XI: the driver only publishes a result from a completed depth.

---

## Transposition tables and Zobrist hashing@Chess Programming Wiki

- **captured:** 2026-09-18
- **source:**
  - https://www.chessprogramming.org/Transposition_Table
  - https://www.chessprogramming.org/Zobrist_Hashing
- **why consulted:** Decide whether and how to memoize positions reached by different move orders.

### Relevant API shape

```text
entry = { key signature, best move, depth, score, bound in {exact, lower, upper}, age }
replacement: always-replace / depth-preferred / two-tier buckets
Zobrist: XOR of per-(feature,square) random 64-bit keys, updated incrementally.
64-bit keys are considered sufficient; birthday collisions become probable near 2^32 positions.
```

### Gotchas, rate limits, versioning

- Terminal (win/loss) scores that encode distance-to-end must be re-based when read back at a different depth.
- Key collisions must not be able to produce an illegal move: any stored move is re-validated against the current position.

### Decision impact

- Ties to ADR 0005 and the spec non-goal on unjustified optimizations: adopt only after a measured transposition rate justifies it.

---

## Move ordering@Chess Programming Wiki

- **captured:** 2026-09-18
- **source:** https://www.chessprogramming.org/Move_Ordering
- **why consulted:** Make alpha-beta cutoffs happen early.

### Relevant API shape

```text
order: hash/PV move first; then captures (MVV-LVA/SEE); then killer moves;
then history-heuristic score. Cut-nodes fail high on the first move ~90% of the time.
```

### Gotchas, rate limits, versioning

- Battlesnake has no captures; the analogous "forcing" signals are food, head-to-head threats, and moves that shrink the opponent's reachable space.

### Decision impact

- Ties to `plan.md` §Module layout (`lookahead::ordering`): previous-iteration best move, killer moves per ply, and a history table over (heading) pairs.

---

## Simultaneous-move alpha-beta@AAAI 2012

- **captured:** 2026-09-18
- **source:** https://ojs.aaai.org/index.php/AAAI/article/view/8148 (Saffidine, Finnsson, Buro: "Alpha-Beta Pruning for Games with Simultaneous Moves")
- **why consulted:** Battlesnake is a simultaneous-move game; decide how to search it.

### Relevant API shape

```text
stacked matrix games: at each state both players choose simultaneously; sound
pruning maintains upper/lower payoff bounds per state and prunes dominated
actions using linear-program feasibility; targets Nash-equilibrium values.
```

### Gotchas, rate limits, versioning

- Exact simultaneous solving needs an LP feasibility check per node; that cost competes directly with raw node throughput under a ~380 ms budget.
- The common cheaper alternative serializes the turn conservatively (we commit first, the opponent answers knowing it), which yields a lower bound on the true game value.

### Decision impact

- Ties to ADR 0003: start with conservative serialization (lower bound, cheap, alpha-beta compatible); record the matrix-solving refinement as a measured follow-up rather than assuming its benefit.

---

## Voronoi territory in two-player territory games@Google AI Challenge 2010

- **captured:** 2026-09-18
- **source:**
  - https://www.a1k0n.net/2010/03/04/google-ai-postmortem.html
  - https://quotenil.com/google-ai-challenge-2010-results.html
- **why consulted:** Find a public, well-understood positional signal for a two-agent, trail-leaving game (Tron), the closest classical analogue of a Battlesnake duel.

### Relevant API shape

```text
Voronoi territory: cells each agent reaches strictly first; territory difference
predicts the eventual outcome; iterative-deepening minimax with alpha-beta over
a Voronoi leaf evaluation was a common strong 1v1 design; a separate endgame
routine takes over when the agents are in disjoint regions; articulation points
and "chamber" trees refine the longest-path estimate; discard an incomplete ply
when time expires.
```

### Gotchas, rate limits, versioning

- Tron trails never disappear; Battlesnake tails do (and lengthen with food), so pure Tron territory counts must be adjusted for time-varying obstacles.
- Head-to-head ties and the "longer wins" rule change who owns contested cells.

### Decision impact

- Ties to ADR 0004: time-aware territory (tail release modelled), an endgame estimator for separated agents, and a distinct terminal-scoring stage.

---

## Bitboard set operations and neighbor shifts@Chess Programming Wiki

- **captured:** 2026-09-18
- **source:** https://www.chessprogramming.org/Bitboards
- **why consulted:** Represent the 11x11 board so territory fills run as a handful of word operations.

### Relevant API shape

```text
set operations via AND/OR/NOT; directional neighbour generation via shifts;
horizontal shifts need edge (file) masks to prevent wraparound; fill algorithms
propagate sets across the board without per-square iteration.
```

### Gotchas, rate limits, versioning

- 121 cells do not fit a 64-bit word; `u128` holds them with 7 spare bits that every operation must mask off.

### Decision impact

- Ties to `plan.md` §Data model: a single `u128`-backed cell set with row stride 11 and explicit left/right edge masks.

---

## snork agents@wrenger/snork README (MIT)

- **captured:** 2026-09-18
- **source:** https://github.com/wrenger/snork (README only)
- **why consulted:** Publicly documented methodology of an MIT-licensed peer, for capability inventory and sparring-opponent identity only.

### Relevant API shape

```text
agents: random, area-control ("very fast"), Tree (minimax with multiple
heuristics), Flood (2nd place, Elite Division, Winter Classic 2021), Mobility.
Hyperparameters are configurable on startup; defaults come from Bayesian
optimization campaigns; a fast simulator supports heuristic evaluation.
```

### Gotchas, rate limits, versioning

- Automated hyperparameter search is a spec non-goal here; weights are tuned by measured, reviewed sparring iterations instead.

### Decision impact

- Ties to spec criterion 8 and `contracts/opponent-roster.yaml`: Flood and Shapeshifter are launched as black-box HTTP opponents only.

---

## rules-core rules core@efed780

- **captured:** 2026-09-18
- **source:** `../rules-core` at commit `efed780`; its `.specs/001-safe-move-api/evidence/latency.md` and `rules-conformance.md`.
- **why consulted:** Define exactly what is reused and what must be new.

### Relevant API shape

```text
pub: domain::{board::{Coordinate, CellIndex, BoardMask}, state::{TurnState, SnakeState},
      simulation::{Direction, resolve_turn, JointMoves, TurnResolution}},
     application::{clock::{Clock, MonotonicInstant, response_deadline},
                   decision::{decide_within_deadline, decide_unsupported}},
     transport::dto::{TurnRequestDto, classify, Scope, to_turn_state}
Measured (its evidence): ~107 us for a 256-joint-action decision (about 0.42 us
per resolve_turn call, i.e. ~2.4 million transitions/second single-threaded) and
~8.7 million nodes/s aggregate at concurrency 16; 93.75% branch coverage on its
simulation module; resolver agreement with the official CLI on 8 scenarios.
```

### Gotchas, rate limits, versioning

- `resolve_turn` owns `String` ids and `Vec` bodies and allocates on every call. That is correct and adequate for one turn, but deep search multiplies the call count by orders of magnitude.
- The dependency is a path dependency on an unpublished crate; the exact commit is recorded here and in the dependency contract so drift is detectable.

### Decision impact

- Ties to ADR 0002: reuse as reference model, fallback engine, and wire-parsing layer; add a compact allocation-free search kernel whose every transition is differentially tested against `resolve_turn`.

---

## Web and runtime crates@same pins as the sibling

- **captured:** 2026-09-18
- **source:** sibling `Cargo.toml` and `research.md` (axum 0.8.9, tokio 1.53.1, serde 1.0.229, serde_json 1.0.151, tracing 0.1.44, tracing-subscriber 0.3.23, proptest 1.11.0, tower 0.5.3), all already reviewed under the sibling's Article XIII.
- **why consulted:** Avoid introducing new dependencies or versions.

### Relevant API shape

```text
No new runtime crates. Zobrist keys are generated by a const splitmix64 sequence;
no random-number crate is needed.
```

### Gotchas, rate limits, versioning

- Cargo unifies the reused crate's exact pins (`=x.y.z`); this workspace must not request incompatible versions.

### Decision impact

- Ties to Article XIII and `plan.md` §Stack decision.

---

## Notes

- All entries were captured on 2026-09-18 and must be refreshed after 2026-11-17 before a later phase relies on them.
- No entry recommends copying any implementation; every technique above is described at the level of a textbook, wiki, or published paper.
