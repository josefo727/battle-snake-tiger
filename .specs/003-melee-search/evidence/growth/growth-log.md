# Growth iteration log: the engine in four-snake games

Branch `improve/melee-growth`, from `main` at `bc850a1`. Protocol: the placement benchmark of criterion 6 (`spar --melee`, roster `reference/roster-melee.json` with every engine pinned to its own cores, seeds 1 to 30, two games at a time, 80 minutes per measurement). Each iteration is one commit of the change and one commit of its result. The yardstick is the mean placement, with the per-game placements and the lengths at the end of each game as leading indicators.

**How much these numbers can say.** Thirty games give a mean placement with a 95% interval about ±0.4 wide; a change must move the mean by about 0.5 to be visible. The baseline is the profile committed on `main`.

## Diagnosis (platform games of 2026-09-19, 17 four-snake games after the last deployment)

- 14 wins, 3 losses; the losses came in the one-against-one phase against a rival 2 to 18 segments longer.
- At the end of the 10 games longer than 100 turns the engine was shorter than the last rival in 7; of 48 rival eliminations it caused 12, the rest were the rivals' own errors. It outlasts rather than kills, and it does so because it does not grow: the hunger term only acts when health is low and a segment of length was worth 2.5 cells of territory.

## Baseline (main, `bc850a1`): report `../melee-report.json`

- Mean placement: tiger 2.300, baseline 3.767, Sansón 3.267 in the tiger's games. Placements 1:9 2:8 3:8 4:5.
- The engine's games lasted 477 turns on average.

## Iteration 1: a segment worth ten cells (duel length 250 to 1,000, melee standing 250 to 1,000)

- Change commit `1ad1793`, reverted by `c70623d`. Transcripts `target/sparring-melee` (the report file was not written: the CLI exited 1 on the baseline's seed 27 with an empty transcript, so the baseline seating has 26 games; the engine's 30 are complete and the placements below are computed from the transcripts with the same rule as the report).
- Result: **mean placement 2.433** (±0.41; placements 1:8 2:8 3:7 4:7), against 2.300 (1:9 2:8 3:8 4:5) on `main`. Sansón 2.867 in the engine's games (was 3.267). Baseline 3.769 over its 26 games.
- Growth (the leading indicator): length at turn 50 is 5.7 against 8.2 for the longest rival, at turn 100 7.6 against 12.2; the engine is still out-eaten two to one early, exactly as before the change. Games last 443 turns on average (477 before).
- Reading: the weight of a segment does not make the engine go for food; pellets are mostly beyond the four-ply horizon and nothing in the valuation pulls toward them while health is high. Two more last places and a slightly worse mean, within noise but in the wrong direction. Not adopted, reverted.
- Decision for iteration 2: change behaviour, not weights: a term that pulls toward the nearest pellet the engine reaches before every rival, from turn 0 and independent of health.

## Iteration 2: the appetite term at 300 per turn of nearness (stopped after 14 games)

- Commits: Red `8c4a8de`-style test and Green `14ddd25` (appetite: `REACH - distance` to the nearest pellet we reach first, weight 300, `REACH` 12).
- Result after 14 of 30 engine games (the run was stopped; transcripts discarded with the next run): mean placement 2.571 (1:3 2:4 3:3 4:4), alive at the end in 3; **length at turn 50: 3.9** against 8.0 for the longest rival (5.7 before), at turn 100: 5.2 against 11.4.
- Reading: the term made the engine eat *less*, not more. Being one step from an owned pellet is worth up to 11 x 300 = 3,300, and eating it makes that value vanish (the pellet is gone) for a gain of one segment worth 250: the search learned to hover beside food. The same trap explains the duel branch's failed appetite (iteration 2 there). A proximity term only works when a segment is worth more than the whole proximity range.
- Decision: keep the term but invert the sizes: a segment worth 1,000 (standing) and the full appetite range worth less than one segment (80 per turn, at most 960), so eating always beats hovering and the gradient still points at the food.

## Protocol change (2026-09-19, after iteration 2): the yardstick is Shapeshifter

The operator's aim is to be better than or equal to Shapeshifter (the strong build: `tt`, `parallel_search`, `mcts_fallback`) in four-snake games, so from here the benchmark seats Shapeshifter, `flood-a` and `flood-b` beside the engine (`reference/roster-shapeshifter-melee.json`, every engine pinned to its own two physical cores as before) and plays the challenger's seating only (`spar --melee --challenger-only`, 30 seeds, about 40 minutes). The verdict is the engine's mean placement against Shapeshifter's in the same 30 games (not worse), with wins and losses beside it. Iteration 3 was stopped after four games against Sansón and is re-measured under this protocol; `main`'s profile is measured the same way for the baseline.

## Iteration 3: appetite 80 (below one segment), standing 1,000, against Shapeshifter

- Change commit `e6779cf`. Report `iter-3-vs-shapeshifter.json` (30 games, challenger seating, seeds 1 to 30, Shapeshifter + two Flood).
- Result: **tiger 4 won, 26 lost; mean placement 2.833** (±0.39; 1:4 2:8 3:7 4:11). **Shapeshifter in the same games: 11 won; mean 2.067** (±0.36; 1:11 2:9 3:7 4:3). Tiger placed ahead of Shapeshifter in 9 games, behind in 21. Verdict: NOT MET.
- Growth: length at turn 50 is 5.4 against 8.6 for the longest rival, at turn 100 7.3 against 12.0: unchanged by the appetite term at this size. Games last 516 turns on average.
- Reading: neither the segment weight (iteration 1) nor a food gradient (2, 3) changes how much the engine eats, so the cause is not in the weights but in the search: with three paranoid opponents, any pellet a rival could also reach is judged lost or lethal (an equal-length rival "will" trade heads with us, which the minimizing layer scores as a wipe-out), so the engine only takes uncontested food. Shapeshifter contests food and grows to 8.6 by turn 50.
- Decision: iteration 4 changes the opponent model, not the weights: a rival's reply that trades heads at equal length (its own certain death for ours) is not considered, at any ply; strictly longer rivals keep the threat. The profile of `main` is measured under this protocol first, for the reference.

## Reference: the profile of `main` against Shapeshifter (same protocol)

- Engine built from `main` (`bc850a1`) in a worktree; report `main-vs-shapeshifter.json`.
- Result: **tiger 3 won, 27 lost; mean placement 2.883** (±0.37; 1:3 2:7 3:8 4:11). **Shapeshifter 14 won; mean 1.850** (1:14 2:7 3:6 4:2). Tiger ahead of Shapeshifter in 5 games, behind in 24. Winners: Shapeshifter 14, Flood 12 (6 + 6), tiger 3, one draw.
- Growth: length at turn 50 is 5.2 against 8.5 for the longest rival, at turn 100 6.3 against 12.3. Games last 478 turns.
- Reading: the deployed engine is last of the four at this table; iteration 3 (2.833) is indistinguishable from it. Flood is a first-rank opponent here (12 wins to Shapeshifter's 14 across two seats).

## Iteration 4: a rival no longer than us does not trade heads (rival model), against Shapeshifter

- Change commit `c478395`. Report `iter-4-vs-shapeshifter.json`.
- Result: **tiger 6 won, 24 lost; mean placement 2.733** (±0.41; 1:6 2:6 3:8 4:10). **Shapeshifter in the same games: 9 won; mean 2.483** (1:9 2:5 3:6 4:9). Tiger ahead of Shapeshifter in 13 games, behind in 17 (the reference: 5 and 24). Winners: Shapeshifter 9, Flood 14 (8 + 6), tiger 6, one draw.
- Growth: length at turn 50 is 5.9 against 8.0 (reference 5.2 against 8.5), at turn 100 7.7 against 12.0. Games last 479 turns.
- Reading: the first change that moves every indicator the right way: twice the wins of the reference, a better mean, and Shapeshifter's own placement in these games worse by 0.6 (it is contested now). Still short of equivalence (2.73 against 2.48) and the early growth gap remains. Kept.
- Decision: iteration 5 replaces the paranoid replies below the root with a policy per rival (it plays for itself: kill when longer, eat, walk to food, keep space), so our own line is searched deeper with the same time.

## Iteration 5: rivals play their own policy below the root (rejected)

- Commits: Red `4a6d905`, Green `c71fc64`, both reverted. Report `iter-5-vs-shapeshifter.json`.
- Result: **tiger 2 won, 28 lost; mean placement 3.367**; Shapeshifter 13 won, mean 1.933. Tiger ahead of Shapeshifter in 3 games, behind in 27. Length at turn 50: 5.0. Games last 504 turns.
- Reading: the worst profile measured. Predicting one reply per rival made the deeper line worthless: the real rivals do not play the policy, and a search that trusts it walks into cells the paranoid model would have refused. The extra depth bought nothing because the model under it was wrong. The paranoid tree with iteration 4's rule (no equal-length head trades) stays.
- Decision: revert to iteration 4 and continue with the larder (pellets we reach first as value), then root parallelism under an evidence gate.

## Iteration 6: the larder (pellets we reach first, 400 each; appetite 40), on iteration 4

- Change commit `398a72b`. Report `iter-6-vs-shapeshifter.json`.
- Result: **tiger 5 won, 25 lost; mean placement 2.717** (±0.36; 1:5 2:5 3:13 4:6). Shapeshifter 10 won, mean 2.483; tiger ahead of it in 14 games, behind in 16. Flood-a 6 won (tiger ahead 14, behind 15), flood-b 9 won (ahead 10, behind 20).
- Growth: length at turn 50 is 6.1 against 7.8 (iteration 4: 5.9 against 8.0), at turn 100 7.9 against 11.6. Games last 482 turns.
- Reading: indistinguishable from iteration 4 in the mean (2.717 against 2.733) but with fewer last places (6 against 10) and slightly more early growth. Kept: no cost, small gains in the right direction. The gap to Shapeshifter (0.23 places) is now within the noise of 30 games; the gap to flood-b is the larger one.
- Decision: iteration 7 is depth, not valuation: the root split over threads, with the profile as the gate (at least half a ply more with two threads, the VPS's count) before the benchmark.

## Iteration 7: the root split over threads (gate not passed, kept opt-in)

- Change commit `b01afc2` (`SEARCH_THREADS`, default 1). Gate: `PROFILE_MELEE_THREADS={1,2,4} scripts/run-profile melee` on the 100 four-snake midgames, 370 ms each.

| Threads | Depth min / median / max (mean) | Nodes per second | Slowest decision |
|---------|----------------------------------|------------------|------------------|
| 1 | 1 / 4 / 9 (4.02) | 345,410 | 373.0 ms |
| 2 | 1 / 4 / 8 (3.91) | 531,604 | 373.2 ms |
| 4 | 1 / 4 / 8 (4.07) | 756,582 | 373.3 ms |

- Reading: two threads search 1.5 times the nodes and four threads 2.2 times, but the completed depth does not move (the gate asked for half a ply with two threads). Two causes: each lane searches with an open window, so the pruning the sequential root gets from its best heading's bound is lost; and one more ply costs about four times the nodes, so 1.5 to 2.2 times is not enough to finish it inside the allowance. Not adopted as the default; the code stays available for a later Lazy-SMP attempt once a transposition table is measured.
- Also seen: single-thread throughput fell from 649,059 nodes per second (feature 003 evidence) to 345,410 since the growth terms were added, because Territory, Hunger, Appetite and Larder each run the four-source fill on the same leaf. Iteration 8 computes the survey once per leaf; that is worth more depth than the threads were.

## Iteration 8: one survey per leaf (speed, no score change)

- Change commit `494fd0d`. Profile (`scripts/run-profile melee`, one thread): **1,159,270 nodes per second** (was 345,410 with the growth terms, 649,059 at the feature's acceptance), depth min / median / max 1 / 4 / 9, **mean 4.54** (was 4.02): half a ply more, what the root split could not buy. Benchmark against Shapeshifter follows.
- Benchmark (report `iter-8-vs-shapeshifter.json`): **tiger 4 won, 26 lost; mean placement 2.717** (±0.37; 1:4 2:7 3:10 4:8). Shapeshifter 12 won, mean 2.233; tiger ahead of it in 13 games, behind in 17. Flood-a 5 won (ahead 13, behind 16), flood-b 8 won (ahead 12, behind 18). Length at turn 50: 5.9 against 8.1; games last 523 turns.
- Reading: half a ply more changed nothing measurable: the same 2.717 as iteration 6. Iterations 4, 6 and 8 sit on a plateau at 2.72 against Shapeshifter's 2.2 to 2.5; depth is not the limiting factor at this point, the opponent model and the finishing are. Kept (it is only faster).
- Decision: iteration 9 is a hybrid opponent model: a rival whose head is near ours (within four cells) keeps the paranoid replies (with iteration 4's rule), a rival far from us plays its own policy (the reverted iteration 5 policy) below the root. Iteration 5 failed because it trusted rivals in contact; here contact stays paranoid.

## Iteration 9: hybrid rival model, far rivals play their policy (rejected)

- Change commit `8e3e9d9`, reverted. Report `iter-9-vs-shapeshifter.json`.
- Result: **tiger 3 won, 27 lost; mean placement 3.000** (1:3 2:4 3:13 4:10). Shapeshifter 14 won, mean 1.933; tiger ahead of it in 9 games, behind in 21. On the same seeds `main` had 1 won and a mean of 2.98, so the hybrid is no worse than `main` but clearly worse than iterations 4, 6 and 8 (2.72).
- Reading: even rivals four cells away punish a search that predicts them; the paranoid tree is what keeps the engine alive in crowded boards, and depth beyond it does not pay. The opponent model is settled at iteration 4's rule; no more prediction of rivals.
- Decision: iteration 10 is the finisher: when we are longer than a rival in contact, value the exits its head has left and the territory it owns, so the search closes on shorter rivals the way Shapeshifter does.

## Iteration 10: the finisher (exits and territory denied to a shorter rival in contact, 150 each)

- Change commit `9519260`. Report `iter-10-vs-shapeshifter.json`.
- Result: **tiger 7 won, 23 lost; mean placement 2.733** (±0.40; 1:7 2:2 3:13 4:8). Shapeshifter 14 won, mean 2.067; tiger ahead of it in 11 games, behind in 19. Flood-a 5 won (ahead 12, behind 18), flood-b 4 won (ahead 15, behind 15). Length at turn 50: 5.7 against 8.5; games last 523 turns.
- Reading: the most wins of any profile (7; `main` 3, iterations 4 to 8 between 4 and 6) with the same mean as the plateau: the term converts some second places into wins and some into thirds. Within the noise of 30 games either way. Kept for the wins.
- Decision: iteration 11 measures the duel-phase weights alone (the one-against-one endgame is where the losses to a longer rival come, and the duel branch's adopted profile, length 1,000, was only ever measured inside iteration 1's package).

## Iteration 11: the duel-phase profile alone (length 1,000, win 1,000,000)

- Change commit `a9354db`. Report `iter-11-vs-shapeshifter.json`.
- Result: **tiger 8 won, 22 lost; mean placement 2.617** (±0.42; 1:8 2:4 3:9 4:8). **Shapeshifter 8 won, mean 2.417**; tiger ahead of it in 13 games, behind in 16. Flood-a 7 won (ahead 16, behind 14), flood-b 7 won (ahead 12, behind 18). Length at turn 50: 6.1 against 8.6; games last 462 turns.
- Reading: the best profile measured: most wins (8, level with Shapeshifter in these games), best mean, and the smallest gap to Shapeshifter (0.20 places, inside the ±0.4 noise of 30 games). The one-against-one endgame, where the losses to a longer rival came from, is where the duel weights act. Kept.
- Where the branch stands against `main` on the same 30 seeds: wins 8 against 3, mean 2.62 against 2.88, ahead of Shapeshifter 13 times against 5. Not yet "no worse than Shapeshifter" by the mean (2.62 against 2.42), but within noise; a 60-seed run would be needed to tell, and the duel regression check of feature 001 (criterion 8) has not been rerun for this profile.

## Summary of the growth iterations (Shapeshifter + two Flood, seeds 1 to 30)

| Iteration | Change | Tiger won | Mean | Shapeshifter mean | Ahead / behind | Outcome |
|-----------|--------|-----------|------|-------------------|----------------|---------|
| main | reference | 3 | 2.88 | 1.85 | 5 / 24 | |
| 3 | appetite below a segment | 4 | 2.83 | 2.07 | 9 / 21 | kept, then superseded |
| 4 | no equal-length head trades | 6 | 2.73 | 2.48 | 13 / 17 | kept |
| 5 | rivals play a policy | 2 | 3.37 | 1.93 | 3 / 27 | reverted |
| 6 | larder | 5 | 2.72 | 2.48 | 14 / 16 | kept |
| 7 | root split over threads | profile: no depth gained | | | | opt-in only |
| 8 | one survey per leaf | 4 | 2.72 | 2.23 | 13 / 17 | kept (speed) |
| 9 | hybrid rival model | 3 | 3.00 | 1.93 | 9 / 21 | reverted |
| 10 | finisher | 7 | 2.73 | 2.07 | 11 / 19 | kept |
| 11 | duel-phase weights | 8 | 2.62 | 2.42 | 13 / 16 | kept |

## Iteration 12: a depth starts by the predicted cost of the next iteration (kept)

- Change commit `f5b867c`, replacing the fixed 40% cut. Gates: duel latency p99 371.0 ms (max 373.6, depths 8 to 13), melee latency p99 372.6 ms (max 374.8, depths 3 to 5), both at concurrency 16 with a reduced workload (`latency-duel-iter-12.md`, `latency-melee-iter-12.md`); melee profile mean depth 4.49 (4.54 before, same within noise), slowest decision 370.9 ms.
- Light batch (Shapeshifter + Mobility + one-turn, seeds 1 to 10, `iter-12-light.json`): **tiger 3 won, 7 lost, mean 1.950** (was 2.150); Shapeshifter 7 won, mean 1.300. Ahead of it in 3 games, behind in 7: the same head-to-head as before, better placements. Games longer (711 and 653 turns in two wins).
- Reading: the platform's unused half-budgets are gone (the slowest decision and the p99 unchanged, so nothing overruns), the placement improves a little, the duel finals against Shapeshifter still fall his way (4 of 5 when both reach the one-against-one). Kept.
- On the transposition-table gate: the duel leaf did not change, so the 2.7% usable repeats measured in feature 001 stand; not re-measured.
- Decision: iteration 13 gives the duel the same growth and finishing terms the melee has (appetite below a segment, larder, finisher), with one survey per leaf as in iteration 8; measured by the light batch and the 12-seed duel benchmark against the strong build.

## Iteration 13: the duel growth term (appetite, larder, finisher, antisymmetric) (rejected)

- Change commit `1238768`, reverted. Reports `iter-13-light.json`, `iter-13-duel-strong.json`.
- Light batch (Shapeshifter + Mobility + one-turn, seeds 1 to 10): **tiger 2 won, 8 lost, mean 2.000** (iteration 12: 3 won, 1.950); Shapeshifter 7 won, mean 1.250; ahead of it in 2, behind in 7. Duels against the strong build, seeds 1 to 12: **5 won, 7 lost** (0.417), the same as the duel branch's best (5 of 12); criterion 8 still met (baseline 0 of 24).
- Reading: no gain on either yardstick and a slower duel leaf (one more fill); the duel finals are not lost for lack of food or of a finishing bonus. Reverted.
- Where this leaves the work: iterations 4, 6, 8, 10, 11 and 12 are in `main`; against Shapeshifter + two Flood the engine went from 3 wins and 2.88 to 8 wins and 2.62 (Shapeshifter 2.42 in those games); with weak fillers the duel finals against the strong build still fall his way (about 1 in 4). The valuation-and-weights lever is exhausted at this sample size; the next gains need bigger pieces (a transposition table with a fresh gate on the fast leaf, or a different endgame search), each a day of work with its own measurement.
