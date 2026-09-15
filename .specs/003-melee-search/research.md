# Research - 003-melee-search

## Research method

Captured 2026-09-19. Public literature and the official Battlesnake rules only: the multi-player search techniques below come from peer-reviewed papers and their public summaries, and the N-snake collision rules from the official docs plus the behaviour of the reused sibling resolver (`resolve_turn`), which this project already treats as the executable oracle (ADR 0002). No reference engine's source was consulted (Article VIII, `PROVENANCE.md`). Sansón, the operator's previous engine, is used only as a black box (a Docker image) in the placement benchmark.

## Battlesnake rules for three and four snakes@captured-2026-09-19

- **captured:** 2026-09-19
- **source:** https://docs.battlesnake.com/rules (WebFetch); `../rules-core/src/domain/simulation.rs` (the reused resolver, already the oracle of feature 001)
- **why consulted:** Fix the elimination rules the N-snake kernel must reproduce when more than two heads meet.

### Relevant API shape

```text
Resolution (docs): moves applied to every snake at once (head added, tail
removed, health - 1); food consumed (health reset to 100, one segment added on
the tail); eliminations: health <= 0, out of bounds, self collision, collision
with another snake's body, head-to-head lost. "The longer Battlesnake will
survive and the shorter will be eliminated. If both Battlesnakes are the same
length, then both are eliminated." Eliminated snakes are removed from the board.

Oracle (resolve_turn, N snakes): for every cell with two or more heads, a snake
loses unless it is strictly the longest there; if several share the longest
length they all lose. A snake that left the board has no head (no head-to-head)
but its moved body still blocks. Body collisions look at every snake's
post-move segments (stacked tails included, new heads excluded).
```

### Gotchas, rate limits, versioning

- The public rules text does not say what happens when three heads meet; the oracle's rule (strictly longest survives, ties eliminate every tied snake) is what the kernel reproduces and what the differential test checks.
- Elimination causes are ordered in the oracle (health, bounds, body, head-to-head) but only survival matters to search; the kernel does not model causes.
- Dead snakes vanish from the board on the next turn: their cells become free immediately for the following move.

### Decision impact

- Ties to `plan.md` §Kernel: `MeleeBoard::advance` resolves all seats at once with these rules; the differential property against `resolve_turn` runs on generated three- and four-snake positions.

---

## Paranoid search and max^n@Sturtevant & Korf, AAAI 2000

- **captured:** 2026-09-19
- **source:** https://cdn.aaai.org/AAAI/2000/AAAI00-031.pdf ("On Pruning Techniques for Multi-Player Games"); https://en.wikipedia.org/wiki/Paranoid_algorithm
- **why consulted:** Choose the backup rule for a search with three opponents.

### Relevant API shape

```text
max^n: each node backs up a vector of N scores; each player maximizes its own
component. Only shallow alpha-beta pruning applies, so the tree stays close to
b^d.

paranoid: the N-player game is reduced to a two-player game in which every
opponent minimizes the root player's score (a coalition against the root).
Standard alpha-beta then applies to the whole tree, with the usual best case
of about b^(d/2) leaves under good ordering. The value is a lower bound on
what the root player can secure.
```

### Gotchas, rate limits, versioning

- Paranoid is pessimistic: it may refuse lines that are fine against opponents who play for themselves, and can look timid in crowded positions.
- max^n cannot be pruned deeply, which at 256 joint moves per ply makes it far shallower than paranoid in the same time.

### Decision impact

- Ties to `plan.md` §Move generation and search and ADR 0007: paranoid alpha-beta over serialized layers (ours, then each opponent in seat order), because it keeps the whole feature-001 machinery (fail-soft windows, killers, history, iterative deepening) valid without change and gives the deepest search per millisecond. Its pessimism is a measured risk (placement benchmark).

---

## Best Reply Search@Schadd & Winands, IEEE TCIAIG 2011

- **captured:** 2026-09-19
- **source:** https://dke.maastrichtuniversity.nl/m.winands/documents/BestReplySearch.pdf (DOI 10.1109/TCIAIG.2011.2107323)
- **why consulted:** A cheaper alternative to paranoid when the opponent layers are the cost.

### Relevant API shape

```text
BRS: at each opponent turn only one opponent moves, the one with the strongest
reply; the other opponents pass. The root player's moves therefore appear more
often along a line of fixed depth, giving longer-term planning. Reported
stronger than max^n in every tested game and than paranoid in two of three.
```

### Gotchas, rate limits, versioning

- Passing is illegal in Battlesnake (every snake must move every turn), so BRS needs a stand-in move for the passing opponents; the paper's variant BRS+ uses a static move ordering for them. That approximation is untested here.

### Decision impact

- Ties to `plan.md` §Risks: recorded as the follow-up if the melee profile shows depth below 4 at 370 ms, or if the placement benchmark shows paranoid timidity. Not adopted first because the stand-in move for passing snakes needs its own evidence.

---

## Opponent move pruning in paranoid search@public technique

- **captured:** 2026-09-19
- **source:** https://dl.acm.org/doi/10.1145/3402942.3402957 ("Opponent-Pruning Paranoid Search", FDG 2020, abstract); the sibling's own compose notes for Sansón record the same idea as "reduced rival replies below the first one" (read only as the deployment file `/opt/battle-snake/compose.yml`, not source)
- **why consulted:** Whether cutting opponent replies below the root is an established, safe reduction.

### Relevant API shape

```text
Opponent-pruning paranoid search limits the opponents' considered moves
(by a static ordering or a filter) below the root while the root player keeps
its full move set; it trades some paranoid exactness for depth.
```

### Gotchas, rate limits, versioning

- The filter must not remove a move that could hurt the root player; discarding an opponent's certain suicides (off board, into a body that stays) is safe because such a move cannot change our survival, only the opponent's.

### Decision impact

- Ties to `plan.md` §Move generation and search: opponents below the root only try headings that do not kill them outright; an opponent with no safe heading keeps all four (its head-to-head still matters).

---

## Sansón as a black-box opponent@image battle-snake:7212509

- **captured:** 2026-09-19
- **source:** `docker image inspect` of the local image; `/opt/battle-snake/compose.yml` on the VPS (read, never edited); `PROVENANCE.md`
- **why consulted:** How to run the previous engine in the placement benchmark without reading it.

### Relevant API shape

```text
image: battle-snake:7212509 (same id as battle-snake:current on the VPS)
entrypoint /usr/local/bin/battle-snake, listens on 3000, user snake
environment on the VPS: SANSON_DEEP=1, SANSON_REDUCE=1, SANSON_LOG_DIR
launch for sparring: docker run --rm --name sanson-<port> -p 127.0.0.1:<port>:3000
                      -e SANSON_DEEP=1 -e SANSON_REDUCE=1 battle-snake:7212509
stop: docker rm -f sanson-<port> (killing the docker client does not stop it)
```

### Gotchas, rate limits, versioning

- The container outlives the `docker run` client process, so the roster needs an explicit stop command (contract `opponent-roster` 1.2.0).
- Sansón uses most of its 500 ms budget; a four-snake game with it takes about 0.5 s per turn, so 60 games (challenger and baseline) take roughly two hours one at a time.

### Decision impact

- Ties to `plan.md` §Sparring: the roster's `Launch` gains an optional `stop` command; the melee benchmark runs one game at a time by default.
