# Spec - 002-game-logging

## Summary

The server can keep its own log on disk, in daily files that survive redeploys, and reports the start and the end of every game it takes part in, so games played on the Battlesnake platform can be reviewed afterwards. Move decisions keep their existing schema; the two lifecycle events are new.

## User story

As the operator of the engine on a shared server, I want a durable, bounded, day-by-day log of every decision and of each game's start and end, so that after a test session I can see what the engine did in each game without depending on the container's own log, which disappears when the container is recreated.

## Acceptance criteria

1. When the environment variable `LOG_DIR` names a directory, every event the server logs (the `listening` line, each `move_decision`, the lifecycle events and errors) is also appended, one JSON object per line, to `LOG_DIR/tiger.log.YYYY-MM-DD` where the date is the UTC day of the event. Without `LOG_DIR` nothing is written to disk and standard output is unchanged.
2. A file is started at the first event of each UTC day; nothing is ever rewritten. At most `LOG_KEEP_DAYS` (default 14) daily files are kept: older `tiger.log.*` files in `LOG_DIR` are deleted when a new day's file is opened, and no other file is touched.
3. A failure to write the log (full disk, permissions, missing directory) never changes an HTTP response, never stops the server and never stops standard-output logging.
4. Each valid `POST /start` produces exactly one `game_started` event with `game_id`, `turn`, ruleset name and version, map, declared timeout, board width and height and the number of snakes; each valid `POST /end` produces exactly one `game_ended` event with `game_id`, the final `turn`, the number of snakes still alive and whether our snake is among them. Neither event carries names, bodies, positions or shouts. The responses of `/start`, `/end` and `/move` are unchanged.
5. The lifecycle events validate against a versioned contract, `contracts/game-lifecycle.schema.json` 1.0.0; the decision events keep validating against `decision-diagnostic` 2.0.0.
6. The deployment on the shared server mounts a host directory as `LOG_DIR`, owned by the container's unprivileged user, and the container still runs read-only.

## Non-goals

- Snake names, board positions or full game reconstruction.
- Log shipping, compression, search or dashboards.
- Any change to move selection.

## Applicable constitution articles

- Article II - Each behavior is built through visible Red-Green-Refactor.
- Article IV - The lifecycle events have a contract before code.
- Article VII - Wall-clock time and the filesystem sit behind ports (a calendar and a directory) so tests need neither a real day nor a real disk failure.
- Article X - The log writer lives in the gateway; inner layers never learn about files.
- Article XIII - No new dependency: the writer uses the standard library and the already-pinned tracing-subscriber.
- Article XIV - Decisions and lifecycle are observable and reviewable after the fact.

## Open questions

None. (Snake names in `game_started` were left out by default, see clarify Q2.)

## Glossary additions

- **Daily file** - `LOG_DIR/tiger.log.YYYY-MM-DD`, the JSON lines of one UTC day.
- **Lifecycle event** - `game_started` or `game_ended`.
