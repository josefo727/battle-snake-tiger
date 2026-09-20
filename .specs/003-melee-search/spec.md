# Spec - 003-melee-search

## Summary

The engine also searches multi-turn in Standard games with three or four snakes, the format the operator actually plays (four snakes per game), so that it is the equal of the engine it replaces (Sansón) in those games. Duels (two snakes) keep the search of feature 001; one snake left and out-of-scope requests keep their fallbacks.

## User story

As the operator playing four-snake games on the Battlesnake platform, I want my snake to look several turns ahead against three, two and one opponents, weighing territory, food, length and the danger of larger neighbours, so that it survives longer and wins more often than a one-turn-safety snake and no less often than my previous engine.

## Acceptance criteria

1. For a valid `POST /move` request declaring Standard rules-module `v1.2.3` (or the local CLI's `cli` identifier), non-wrapped, hazard-free 11x11, exactly three or four active snakes, and a timeout greater than 130 ms, the engine selects its move by evaluating at least one full additional turn beyond the immediate one for every snake, not only immediate survival.
2. The returned move is one of the four platform-valid directions and, on loopback under the same load protocol as feature 001 (16 concurrent requests), the p99 of the response time is at most `game.timeout - 120 ms`.
3. When the allocated budget is exhausted before a deeper search layer completes, the engine returns the move of the deepest fully completed depth, never a partial or arbitrary one; if no depth completes it returns the one-turn safety decision and says so.
4. Each decision reports the completed depth and the positions explored, with `engine_path` `melee_search`; the decision diagnostic contract gains that value in an additive version 2.1.0, and every other field keeps its meaning.
5. Requests with two snakes, one snake or outside the certified scope behave exactly as before (duel search, unsupported fallback), and `GET /`, `/start` and `/end` are unchanged.
6. A fixed, reproducible four-snake benchmark of 30 games with seeds 1 to 30, in which the engine plays against Sansón (the previous engine, run as a black box) and two copies of Flood, reports the engine's mean placement; a second run of the same 30 games with the sibling one-turn baseline in the engine's seat reports its mean placement. The engine's mean placement is strictly better than the baseline's and not worse than Sansón's mean placement in the same games (placement 1 is the last snake alive; snakes eliminated in the same turn share the average of their places).

## Non-goals

- Five or more snakes, other board sizes, wrapped maps, hazards, Royale or Squads.
- Any change to duel play (feature 001) beyond what a shared kernel forces, and no regression of its measured latency and depth.
- Transposition tables, parallel search, learned or fitted weights (each needs profiling or sparring evidence first).
- Snake names or positions in logs.

## Applicable constitution articles

- Article I - every task traces to a criterion here.
- Article II - Red-Green-Refactor with visible Red.
- Article V - clarifications recorded in the clarification log (the bar, the opponents, the order).
- Article VIII - independently authored: public multi-player search literature only, no reference-engine source.
- Article IX - the certified scope grows only to three and four snakes on the same board.
- Article X - search and valuation depend inward only.
- Article XI - deadline safety and a deterministic legal fallback.
- Article XII - the 90 percent branch-coverage floor on the new kernel, valuation and search modules.
- Article XIV - depth, nodes, latency and placement are measured, not asserted.

## Open questions

None.

## Glossary additions

- **Melee** - a supported game with three or four active snakes.
- **Placement** - the rank of a snake in a finished game: 1 for the last alive, then by elimination order backwards; simultaneous eliminations share the average rank.

---

## Closed (reconstructed history)

- Date: `2026-09-16`
- Commit: `3f305a3b1cb7bb03708057042bbd3ca1f45ffc2c`
- Notes: Task commits consolidate the preserved Red-Green-Refactor beats.
