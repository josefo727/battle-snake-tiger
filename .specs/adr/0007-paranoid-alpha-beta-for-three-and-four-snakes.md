# ADR 0007 - Paranoid alpha-beta over serialized seats for three and four snakes

- **Status:** accepted
- **Date:** 2026-09-19
- **Deciders:** José R. Gutierrez
- **Context links:** `../003-melee-search/spec.md` criteria 1, 3, 6; `../003-melee-search/research.md`; ADR 0003

## Context

The operator plays four-snake games. Feature 001 searches only duels; with three or four snakes the engine falls back to one-turn safety and loses to the engine it replaced. A multi-player backup rule is needed that fits the existing search machinery and a 370 ms budget at 256 joint moves per ply.

## Options considered

### Option A - Paranoid alpha-beta (every opponent minimizes our score)

- Pros: plain alpha-beta on the whole tree, so fail-soft windows, killers, history and iterative deepening carry over; deepest search per millisecond; a sound lower bound on what we can secure.
- Cons: pessimistic; can refuse lines that are fine against self-interested opponents.
- Effort / risk: low; the shape is ADR 0003 with more minimizing layers.

### Option B - max^n (each snake maximizes its own component)

- Pros: models self-interest.
- Cons: only shallow pruning; far less depth in the same time; the score vector needs an N-snake valuation from every seat's point of view.
- Effort / risk: moderate; likely too shallow at 256 joint moves.

### Option C - Best-reply search

- Pros: more of our own moves per line; reported stronger than paranoid in some games.
- Cons: passing is not legal in Battlesnake, so the non-replying snakes need a stand-in move whose effect is unmeasured here.
- Effort / risk: moderate; recorded as the follow-up.

## Decision

Option A, with opponents below the root restricted to headings that do not kill them outright (opponent-pruning paranoid search) and a hand-off to the duel valuation when only two snakes remain. Terminal scores rank outcomes by placement so dying later and behind fewer survivors scores higher.

## Consequences

- **Positive:** one kernel and one searcher shape for two, three and four snakes; the duel path is untouched.
- **Negative:** timidity in crowded positions; the placement benchmark (criterion 6) is the check, and Option C is the recorded next step if it fails.
- **Follow-ups:** a melee profile decides whether depth is enough; the benchmark drives the weights.

## Constitution impact

None.

## References

- Research entries: `../003-melee-search/research.md` §Paranoid search and max^n, §Best Reply Search, §Opponent move pruning
