# ADR 0004 - Composable valuation pipeline with time-aware territory

- **Status:** accepted
- **Date:** 2026-09-09
- **Deciders:** José R. Gutierrez
- **Context links:** `../001-duel-search/spec.md` user story; Constitution Articles VI, VIII, X, XII, XIV

## Context

Search reaches finite depth; leaf quality decides play. The initial prompt asks
for strong territory, survival, food, and head-to-head reasoning, and lists
time-aware reachability (future tail release) as a candidate differentiator.

## Options considered

### Option A - One monolithic evaluation function

- Pros: simplest to write.
- Cons: untestable in parts, hard to weight or ablate, violates the
  Open/Closed principle the project wants.
- Effort / risk: low effort, poor evolvability.

### Option B - A pipeline of small assessors, each answering one question, combined by an explicit weight sheet

- Pros: each assessor is unit- and property-testable; contributions are
  reportable for telemetry; new assessors add without editing old ones; weights
  live in one named profile.
- Cons: some indirection cost on the hot path unless assessors are statically
  dispatched.
- Effort / risk: moderate; static dispatch keeps overhead low.

## Decision

Option B, statically dispatched. Assessors: `Dominion` (time-aware Voronoi
territory: cells reached strictly first, with released tail segments becoming
passable at their release turn and equal-arrival cells settled by length),
`Sustenance` (health margin against distance to food we can reach first),
`Enclosure` (when the snakes are in disjoint regions, an estimate of how many
turns each can survive in its own region), `Leverage` (length difference and
head-to-head pressure), and `Finish` (terminal scoring that prefers faster wins
and slower losses). A `WeightSheet` holds all coefficients. Weights are tuned
through reviewed sparring iterations, not automated hyperparameter search (a
spec non-goal).

Design-diff note: publicly, reference engines describe combining "multiple
heuristics" behind a configurable set of hyperparameters; here the decomposition
is by the question each assessor answers (dominion, sustenance, enclosure,
leverage, finish), each an independent type behind one trait, with contributions
exposed as a ledger for diagnostics.

## Consequences

- **Positive:** testable, ablatable, explainable per decision.
- **Negative:** the initial weights are reasoned, not fitted; strength depends on
  measured iteration.
- **Follow-ups:** a sparring iteration log records each weight change and its
  measured effect.

## Constitution impact

None.

## References

- Research entries: `../001-duel-search/research.md` §Voronoi territory, §Bitboard set operations
