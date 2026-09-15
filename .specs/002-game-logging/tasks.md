# Tasks - 002-game-logging

## Legend

Same legend as `001-duel-search`: `R` Red, `G` Green, `F` Refactor, statuses `open | in_progress | closed | skipped`.

Ordering: the calendar, then the file writer and its retention, then the settings, then the lifecycle events and their wiring, then the process and the deployment.

## T001 - Compute UTC calendar dates behind a Calendar port

```yaml
id: T001
status: closed
commits: { reconstructed: bcaa2bc40723af9b4b602c9060bac646b4c92279 }
source-commits: { red: a7d86cc, green: 0230c30, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1 and 2
contract-ref: n/a
constitution-ref: Articles II, VII
```

**Definition of Done**

- `CivilDate::from_unix_seconds` converts Unix seconds to a UTC year, month and day, exactly, from 1970 to 2100 including leap days.
- `CivilDate` formats as `YYYY-MM-DD` and parses it back; anything else is rejected.
- A `Calendar` port answers today's date; the system implementation reads the wall clock and a fixed one serves tests.

**R - Red:** Add a `CivilDate` scaffold returning 1970-01-01 for every input and a table of known dates (the epoch, 2000-02-29, 2026-09-19, 2100-03-01) expecting the real ones; record the mismatch.

**G - Green:** Implement days-from-civil, formatting, parsing and the system calendar.

**F - Refactor:** Skipped - formatting and parsing share no digit helper worth extracting (parsing folds bytes, formatting uses the standard width flags).

**Files**

- `engine/src/gateway/calendar.rs`
- `engine/src/gateway/mod.rs`
- `engine/tests/gateway_calendar.rs`

Note: parsing also rejects the year-0 style sign and any trailing whitespace; the exhaustive walk from 1970 to 2100 doubles as the proof of the leap-year rules.


## T002 - Append log lines to a daily file

```yaml
id: T002
status: closed
commits: { reconstructed: 6b6ec95a50c2556cc7ebea45999b4cfe79022948 }
source-commits: { red: 5287bc9, green: 7e0595a, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1 and 3
contract-ref: n/a
constitution-ref: Articles II, VII, X
```

**Definition of Done**

- `DailyFileWriter` appends each write to `LOG_DIR/tiger.log.YYYY-MM-DD` for the calendar's current day and opens a new file when the day changes, never rewriting an earlier one.
- It works as a `tracing_subscriber` `MakeWriter` and is safe to share between threads.
- A missing directory is created; any I/O error is swallowed, reported as success to the caller, and retried on the next write.

**R - Red:** Add a writer scaffold that writes nowhere and a test with a fake calendar expecting a line in the first day's file and another in the next day's; record the missing files.

**G - Green:** Implement the rotation, append, directory creation and error swallowing.

**F - Refactor:** Skipped - opening and rotating are one step (`open_day`, called when the day differs) and the error handling is a single place (`append`).

**Files**

- `engine/src/gateway/logfile.rs`
- `engine/src/gateway/mod.rs`
- `engine/tests/gateway_logfile.rs`

Note: retention (`keep_days`) is accepted but not applied yet; T003 adds it at the point where a new day's file is opened.


## T003 - Keep only the configured number of daily files

```yaml
id: T003
status: closed
commits: { reconstructed: 34477bb5a0e63426d4bdd724a0620fb9e57c0e2b }
source-commits: { red: 004b4df, green: 810c55c, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 2
contract-ref: n/a
constitution-ref: Articles II, VII
```

**Definition of Done**

- When a new day's file is opened, the oldest `tiger.log.YYYY-MM-DD` files beyond `keep_days` are deleted.
- Only files whose names match the pattern exactly are candidates; every other file and directory in `LOG_DIR` is left alone.
- A deletion failure is swallowed like any other I/O error.

**R - Red:** Add a pruning scaffold that deletes nothing and a test with twenty daily files and one unrelated file expecting the newest fourteen plus the unrelated one; record the survivors.

**G - Green:** Implement the pattern match, ordering and deletion.

**F - Refactor:** Skipped - the `tiger.log.` prefix appears once, inside `parse_file_name` for reading and `file_name` for writing, which are next to each other.

**Files**

- `engine/src/gateway/logfile.rs`
- `engine/tests/gateway_logfile.rs`

Note: the count includes the file just opened, so `LOG_KEEP_DAYS=14` leaves today plus the thirteen newest earlier files.


## T004 - Read LOG_DIR and LOG_KEEP_DAYS

```yaml
id: T004
status: closed
commits: { reconstructed: 94769841c6e29ce56e467243090e4ae4979fe056 }
source-commits: { red: 920d008, green: fec25a3, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 1 and 2
contract-ref: n/a
constitution-ref: Articles II, VII
```

**Definition of Done**

- `Settings` gains an optional `log_dir` and `log_keep_days` (default 14, a whole number of at least 1).
- An empty `LOG_DIR` counts as unset; a bad `LOG_KEEP_DAYS` is refused with an error naming the variable and the value.

**R - Red:** Add fields that are always `None` and 14 and a test expecting `/var/log/tiger` and 7 from a lookup; record the mismatch.

**G - Green:** Parse both variables.

**F - Refactor:** Skipped - LOG_KEEP_DAYS reuses the existing `read` helper; the only added logic is the zero check.

**Files**

- `engine/src/gateway/settings.rs`
- `engine/tests/gateway_settings.rs`

Note: `Settings` stopped being `Copy` (it holds a path); nothing relied on that.


## T005 - Define the lifecycle events and their contract

```yaml
id: T005
status: closed
commits: { reconstructed: 120235ab49cceb6f2ea66844519ffbf7de6f017a }
source-commits: { red: e362464, green: 48abca5, refactor: a0aee53 }
spec-ref: spec.md §Acceptance criteria 4 and 5
contract-ref: contracts/game-lifecycle.schema.json
constitution-ref: Articles II, IV
```

**Definition of Done**

- `LifecycleEvent::started` and `::ended` build the events of contract `game-lifecycle` 1.0.0 from a request, with no names, bodies, positions or shouts.
- Serialized events validate against `contracts/game-lifecycle.schema.json`; `game_started` carries ruleset, map, timeout, board size and snake count, `game_ended` the alive count and whether we are among them.
- A `LifecycleBeacon` port and a tracing adapter emit one `tracing` event per lifecycle event on target `game_lifecycle`.

**R - Red:** Add an event scaffold with empty fields and a schema-conformance test built from a real request; record the missing members.

**G - Green:** Implement the mapping, the port and the tracing adapter.

**F - Refactor:** Replaced the beacon tests' private tracing capture with the shared `support::capture::json_lines`.

**Files**

- `engine/src/gateway/lifecycle.rs`
- `engine/src/gateway/mod.rs`
- `engine/tests/gateway_lifecycle.rs`

Note: the request DTO carries no snake names at all (only ids), so there is nothing to leak even by accident; the started and ended constructors share their identity fields through `about`.


## T006 - Emit lifecycle events from /start and /end

```yaml
id: T006
status: closed
commits: { reconstructed: pending }
source-commits: { red: f990b14, green: ee3cf0c, refactor: skipped }
spec-ref: spec.md §Acceptance criteria 4
contract-ref: contracts/game-lifecycle.schema.json
constitution-ref: Articles II, IV, VII
```

**Definition of Done**

- Each valid `POST /start` emits one `game_started` event and each valid `POST /end` one `game_ended` event through the injected lifecycle beacon.
- The responses of `/start`, `/end` and `/move` are unchanged; a rejected request emits nothing.
- `build_service` takes the lifecycle beacon beside the decision beacon.

**R - Red:** Add router wiring that emits nothing and a test with a recording beacon expecting one event per valid request; record the zero events.

**G - Green:** Emit from the handlers and thread the beacon through `build_service`.

**F - Refactor:** Skipped - the two handlers are three lines each and already share the one `acknowledge` step that holds the emitting and the answer.

**Files**

- `engine/src/gateway/http.rs`
- `engine/src/lib.rs`
- `engine/tests/gateway_http.rs`
- `engine/tests/support/beacon.rs`

Note: `build_service` and `router` gained the lifecycle beacon parameter; `main.rs` passes the tracing beacon. A game outside the certified scope still reports its start because /start acknowledges any syntactically valid game.


## T007 - Write the same log to the daily file from the process

```yaml
id: T007
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 1 and 3
contract-ref: n/a
constitution-ref: Articles II, VII
```

**Definition of Done**

- `main` builds the subscriber as a stdout JSON layer plus, when `LOG_DIR` is set, a second JSON layer over `DailyFileWriter`.
- The real binary started with `LOG_DIR` writes the `listening` line, the decisions and the lifecycle events to today's file and still prints them.
- With `LOG_DIR` unwritable the server still serves and still prints.

**R - Red:** Add a process test that starts the binary with a temporary `LOG_DIR` and expects today's file with the listening line; record the missing file.

**G - Green:** Wire the layers in `main.rs`.

**F - Refactor:** Skipped unless the two layers duplicate their configuration.

**Files**

- `engine/src/main.rs`
- `engine/tests/process_smoke.rs`

## T008 - Mount the log directory on the shared server

```yaml
id: T008
status: open
commits: { red: null, green: null, refactor: null }
spec-ref: spec.md §Acceptance criteria 6
contract-ref: n/a
constitution-ref: Articles II, XIII
```

**Definition of Done**

- `scripts/container-smoke` runs the image with a mounted directory and finds decision and lifecycle lines in today's file.

**R - Red:** Extend the smoke script to require a log file inside a mounted directory and record the failure against the current image.

**G - Green:** Add the mount, the environment and the directory preparation.

**F - Refactor:** Skipped unless the smoke helpers duplicate each other.

**Files**

- `scripts/container-smoke`
- `engine/tests/packaging.rs`

## Coverage matrix

| Acceptance criterion | Owning tasks |
|---|---|
| 1 Daily file with LOG_DIR | T001, T002, T004, T007 |
| 2 Rotation and retention | T001, T002, T003, T004 |
| 3 Failures never affect service | T002, T003, T007 |
| 4 Lifecycle events | T005, T006 |
| 5 Contracts | T005 |
| 6 Deployment mount | T008 |

## Amendments

| Date | Change | Reason |
|------|--------|--------|
