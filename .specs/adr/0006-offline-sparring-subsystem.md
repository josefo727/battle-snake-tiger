# ADR 0006 - Offline sparring subsystem with process and report ports

- **Status:** accepted
- **Date:** 2026-09-09
- **Deciders:** José R. Gutierrez
- **Context links:** `../001-duel-search/spec.md` criterion 8; Constitution Articles VII, XIV

## Context

Criterion 8 requires a reproducible 30-game-per-opponent benchmark against
Shapeshifter and Flood with a comparison to the sibling one-turn engine. Results
must be evidence, not narrative (Article XIV).

## Options considered

### Option A - Shell script around `battlesnake play`

- Pros: quick.
- Cons: untestable parsing, no statistics, no structured report, brittle process
  handling.
- Effort / risk: low effort, weak evidence.

### Option B - A tested Rust tool with `SparringRunner` and `ReportSink` ports

- Pros: testable orchestration, exact seed control, win/loss/draw parsing from the
  official CLI transcript, Wilson-interval reporting, versioned JSON report.
- Cons: more code than a script.
- Effort / risk: moderate.

## Decision

Option B, in the separate `sparring` crate (ADR 0001). `SparringRunner` is a
port over "launch opponents as processes, run one seeded game through the
official CLI, return the transcript"; `ReportSink` is a port over "persist a
versioned report". Reports follow `contracts/sparring-report.schema.json` and
record opponent identity, seeds, engine commit, and environment. Opponents are
launched as black-box HTTP processes per `contracts/opponent-roster.yaml`.

## Consequences

- **Positive:** repeatable, comparable evidence across features.
- **Negative:** opponent builds live outside version control and must be
  provisioned locally.
- **Follow-ups:** provisioning scripts for each opponent, mirroring the sibling's
  fetch-and-verify pattern.

## Constitution impact

None. Implements the boundaries Article VII already declares.

## References

- External sources: https://github.com/BattlesnakeOfficial/rules/blob/main/cli/README.md
