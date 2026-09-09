# ADR 0001 - Cargo workspace with separate engine and sparring crates

- **Status:** accepted
- **Date:** 2026-09-09
- **Deciders:** José R. Gutierrez
- **Context links:** `../001-duel-search/spec.md`, `../001-duel-search/plan.md`, Constitution Articles VII, X, XIII

## Context

The deployable engine has no filesystem, process, or outbound-network dependency
on its move path (Article VII), yet this project must also implement the offline
`SparringRunner` and `ReportSink` boundaries (Article XIV). Those offline tools
spawn processes, write report files, and drive the official rules CLI. Keeping
both in one crate would put process and filesystem code inside the artifact that
ships to the platform.

## Options considered

### Option A - One crate, sparring behind a Cargo feature

- Pros: one manifest; shared test support.
- Cons: process/filesystem code compiles into the same library the server links;
  a feature flag is a weaker guarantee than a crate boundary; Article X's
  dependency direction is enforced by convention, not by the build graph.
- Effort / risk: low effort, weak isolation.

### Option B - Two workspace members: `engine` (library + server binary) and `sparring` (offline tool)

- Pros: the build graph proves the engine never depends on process/filesystem
  sparring code; `sparring` needs nothing from `engine` beyond the wire contract
  it exercises over HTTP; independent dependency review per crate.
- Cons: two manifests and one workspace lockfile to maintain.
- Effort / risk: small and mechanical.

## Decision

Option B. A Cargo workspace at the repository root with members `engine`
(package `tiger-engine`, library `tiger_engine`, binary `tiger-engine`) and
`sparring` (package `tiger-sparring`, binary `spar`). Rust 1.98.1, edition 2024,
pinned by `rust-toolchain.toml`, identical to the sibling project.

## Consequences

- **Positive:** dependency direction is compiler-enforced; the deployable image
  contains only the engine binary.
- **Negative:** shared helpers (report types) must be duplicated or live in
  whichever crate owns them; only `sparring` owns report types.
- **Follow-ups:** the architecture check script covers both crates.

## Constitution impact

None. Realizes Articles VII and X without amendment.

## References

- Research entries: `../001-duel-search/research.md` §Web and runtime crates
