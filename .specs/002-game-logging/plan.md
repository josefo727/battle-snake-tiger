# Plan - 002-game-logging

## Summary

The gateway gains a second tracing layer that appends the same JSON lines to daily files, and two lifecycle events emitted by the `/start` and `/end` handlers through a new port. Everything is standard library plus the already-pinned `tracing-subscriber`; no dependency is added.

## Design

- `gateway::calendar`: `CivilDate` (year, month, day) computed from Unix seconds by the days-from-civil algorithm, and a `Calendar` port (`today() -> CivilDate`) with a system implementation, so tests never depend on the real day.
- `gateway::logfile`: `DailyFileWriter` implements `tracing_subscriber::fmt::MakeWriter`. It holds the directory, the calendar, `keep_days` and the open file of the current day behind a mutex. On each write it asks the calendar for today; when the day changed it opens `tiger.log.YYYY-MM-DD` in append mode and prunes old daily files. Any I/O error is swallowed (criterion 3): the write reports success to tracing and the next write tries again.
- `gateway::lifecycle`: `LifecycleEvent` (serde, contract 1.0.0), `LifecycleBeacon` port and `TracingLifecycleBeacon` (one `tracing::info!` on target `game_lifecycle`), built from the request DTO without names or positions.
- `gateway::http`: `/start` and `/end` parse the request as before, emit the event, answer `{}`.
- `gateway::settings`: `LOG_DIR` (optional) and `LOG_KEEP_DAYS` (default 14, whole number of at least 1).
- `main.rs`: builds the subscriber as a registry with the stdout JSON layer and, when `LOG_DIR` is set, a second JSON layer writing to the daily file.

## Test strategy

- Calendar and file writer: fake calendar, temporary directories; rotation at midnight, retention that touches only `tiger.log.*`, and errors that do not propagate.
- Lifecycle: schema validation against the contract file, exact field presence per event, no name/position leakage, and the HTTP responses unchanged.
- Process: the real binary with `LOG_DIR` set writes decision and lifecycle lines to today's file.
- Container: `scripts/container-smoke` mounts a directory read-only-root-compatible and finds the file.

## Risks

- The log directory being unwritable by uid 10001 (mitigation: deploy script sets ownership; failure is silent by design and visible in the smoke test).
- Time zone confusion (mitigation: UTC only, documented in the file name and criterion 1).
