# ADR 0003 - Iterative-deepening alpha-beta over conservatively serialized simultaneous moves

- **Status:** accepted
- **Date:** 2026-09-09
- **Deciders:** José R. Gutierrez
- **Context links:** `../001-duel-search/spec.md` criteria 1, 3, 4; Constitution Articles VI, VIII, XI

## Context

Battlesnake resolves both snakes' moves simultaneously. The search must return
the best move of the deepest fully completed iteration before the response
deadline (spec criterion 3), and must be as deep as the budget allows because
depth is the main determinant of strength.

## Options considered

### Option A - Alpha-beta with conservative serialization (we commit, the opponent replies knowing it)

- Pros: standard alpha-beta applies; cutoffs work at both layers; cheap per
  node; a sound lower bound on the true simultaneous value.
- Cons: pessimistic (it assumes an opponent who sees our move), which can make
  play timid near head-to-head situations.
- Effort / risk: low; well understood.

### Option B - Exact simultaneous-move alpha-beta (Saffidine et al.)

- Pros: targets the true equilibrium value of each stacked matrix game.
- Cons: requires linear-program feasibility per node, competing directly with
  raw node throughput inside a ~380 ms budget.
- Effort / risk: high; benefit unmeasured for this domain.

### Option C - Monte Carlo tree search

- Pros: handles simultaneous moves and large branching naturally.
- Cons: at a 16-joint-action branching factor and shallow tactical depth, a
  deterministic alpha-beta reaches tactical certainty that sampling only
  approaches; harder to test deterministically (Article XI).
- Effort / risk: moderate; weaker fit for exact short-horizon tactics.

## Decision

Option A as the first implementation. The move tree alternates a maximizing layer
over our four headings and a minimizing layer over the opponent's four headings,
then applies both moves at once. Fail-soft alpha-beta, iterative deepening with a
previous-iteration best move, per-depth killer moves, and a history table for
ordering. An interrupted iteration is discarded. Option B is recorded as a
measured follow-up: it is adopted only if sparring evidence shows timidity
losses that a matrix-value refinement at the root would fix.

Design-diff note: the publicly documented reference engines describe minimax
over an N-player-capable model; this project specializes the tree to exactly two
snakes, so the recursion has a fixed two-layer shape (`Maximizer` then `Minimizer`)
with no player-count parameter, and stores a two-value score instead of a
per-player score vector.

## Consequences

- **Positive:** deterministic, testable, fast; the correctness property
  "alpha-beta equals exhaustive minimax at depth <= 3" is directly checkable.
- **Negative:** pessimism bias; depth quality depends on ordering quality.
- **Follow-ups:** a benchmark task measures depth reached versus ordering
  components; a sparring-driven review decides on Option B.

## Constitution impact

None.

## References

- Research entries: `../001-duel-search/research.md` §Alpha-beta pruning, §Iterative deepening, §Move ordering, §Simultaneous-move alpha-beta
