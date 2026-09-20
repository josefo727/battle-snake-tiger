# Provenance and Independent Authorship

## Policy

This project is independently authored. External projects may inform a capability inventory or act as black-box sparring opponents, but their source expression and architectural shape are not implementation inputs. Code, tests, fixtures, identifiers, module layouts, comments, constants, weights, configuration, and documentation wording are not copied, translated, or adapted. Per the constitution's Article VIII, where this project addresses the same sub-problem as a consulted reference engine, the resulting design must also differ observably in structure, not merely in naming.

Every source that materially affects requirements, contracts, algorithms, architecture, or evaluation must be recorded here before the affected implementation is written.

## Normative product sources

- Battlesnake API Reference: https://docs.battlesnake.com/api
- Battlesnake Webhooks: https://docs.battlesnake.com/api/webhooks
- Battlesnake Game Rules: https://docs.battlesnake.com/rules
- Battlesnake Board object: https://docs.battlesnake.com/api/objects/board
- Battlesnake Standard map: https://docs.battlesnake.com/maps/standard

These sources define observable platform behavior.

## Development method

- spec-tdd-kit.
- Role: phase gates, artifact structure, and Red-Green-Refactor workflow.

## Capability benchmark and sparring opponents only

### Shapeshifter

- https://github.com/JonathanArns/shapeshifter
- Permitted role: high-level capability inventory and black-box sparring opponent (spec.md acceptance criterion 8), driven only through its public `GET /`, `POST /start`, `POST /end`, `POST /move` webhook routes via the official rules engine.
- Prohibited role: source, test, fixture, naming, architecture, constant, weight, comment, or configuration donor.
- License: none found in the repository; treated as all-rights-reserved and used only as a black-box opponent over HTTP.

### `snork` (Flood agent)

- https://github.com/wrenger/snork
- License: MIT (confirmed 2026-09-08 from the repository's `LICENSE` file).
- Permitted role: black-box sparring opponent (spec.md acceptance criterion 8) via Flood's own HTTP webhook routes, run through the official rules engine.
- Prohibited role: source, test, fixture, naming, architecture, constant, weight, comment, or configuration donor for this project's own search or evaluation design, notwithstanding the MIT license permitting reuse — Article VIII's architectural-divergence requirement applies regardless of license terms, because divergence is a product goal here, not only a legal constraint.

## Algorithm and implementation sources

Recorded in `.specs/001-duel-search/research.md` (captured 2026-09-18). Every technique was taken from public textbook, wiki, or paper-level descriptions:

- Alpha-beta pruning, iterative deepening, transposition tables, Zobrist hashing, move ordering, bitboards: Chess Programming Wiki (https://www.chessprogramming.org/).
- Simultaneous-move alpha-beta: Saffidine, Finnsson, Buro, AAAI 2012 (https://ojs.aaai.org/index.php/AAAI/article/view/8148).
- Voronoi territory and endgame handling for two-agent trail games: Google AI Challenge 2010 post-mortem (https://www.a1k0n.net/2010/03/04/google-ai-postmortem.html).
- Battlesnake turn resolution order: https://docs.battlesnake.com/rules.
- `snork` README methodology (documented approach only): https://github.com/wrenger/snork.

## Commit dates

Commit dates in this repository follow a reconstructed chronology; they are not the wall-clock time at which each change was made.
