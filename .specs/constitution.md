# Project Constitution

## Preamble

This project delivers an independently authored, tournament-competitive Battlesnake engine for the `tiger-king` identity, restricted to Standard, non-wrapped 11x11 boards with two to four active snakes. It reuses the already independently authored and conformance-verified rules core from the sibling `rules-core` project as a dependency, and builds a genuine multi-turn search and evaluation layer on top of it. Shapeshifter and other reference engines (for example `snork`'s Flood) may inform a capability inventory and, where authorized, serve as black-box sparring opponents, but neither their source expression nor their architectural decomposition is an implementation input: where this project solves the same sub-problem as a reference engine, the chosen data structures, algorithm shape, naming, and module boundaries are independently derived and are expected to differ observably, not merely be renamed. Measured playing strength against reproducible benchmarks takes precedence over superficial similarity to any reference engine.

## Articles

### Article I - Spec-Anchored Development and Change Traceability

**Statement.** Every production change traces to an approved specification section. Specifications and code evolve together whenever observable behavior changes.

**Rationale.** Explicit traceability prevents implementation drift and makes competitive behavior reviewable.

**Enforcement.** Every task and production commit includes a spec reference; the verify phase rejects untraced changes and stale specifications.

---

### Article II - Test-First Development with Visible Red

**Statement.** No production behavior is written before a test demonstrates a relevant semantic failure. Git history contains a Red commit before the corresponding Green commit for every implementation task.

**Rationale.** Visible Red-Green-Refactor cycles provide design feedback and regression evidence rather than retrospective test coverage.

**Enforcement.** Red commits capture the observed failure; Green commits run the complete suite; verification audits task records and commit ordering.

---

### Article III - Mocks Only at Declared System Boundaries

**Statement.** Test doubles are permitted only at boundaries declared in Article VII. Domain and search tests use real collaborators and observable outcomes rather than interaction scripts.

**Rationale.** This prevents tautological tests and preserves confidence across refactoring, especially for search code where behavior is easy to fake and hard to verify.

**Enforcement.** Test review blocks undeclared mocks; boundary contracts and integration tests exercise real adapters in a hermetic environment.

---

### Article IV - Contract-First External and Platform Integration

**Statement.** The Battlesnake API, the reused rules-core dependency's public interface, any external rules oracle, sparring processes, and any future integration require a versioned contract before consumer code is implemented.

**Rationale.** Stable, explicit contracts detect upstream drift before it affects game behavior, including drift in the sibling rules core this project depends on.

**Enforcement.** Planning creates a contract artifact for every integration; tasks schedule contract tests before dependent consumer tests; verification rejects missing contracts.

---

### Article V - Clarifications Require Explicit User Decisions

**Statement.** A `NEEDS CLARIFICATION` marker may be resolved only through an explicit user decision recorded in the specification or its clarification log.

**Rationale.** Product, competitive, and architectural-divergence decisions must not be silently invented by an implementation agent.

**Enforcement.** The clarify gate requires zero unresolved markers and preserves each decision; verification scans for stale markers.

---

### Article VI - Architecture Decisions Require Durable Records

**Statement.** A decision that affects multiple tasks, selects a dependency, search algorithm, or evaluation composition, introduces a runtime service, changes scope, or amends this constitution requires an Architecture Decision Record.

**Rationale.** Durable records make tradeoffs, and specifically the deliberate divergence from any reference engine's approach, understandable to a later reader.

**Enforcement.** Plan and review gates require an ADR link for every qualifying decision; amendments follow the constitution amendment procedure.

---

### Article VII - Explicit Boundaries and Permitted Test Doubles

**Statement.** The following are the only declared boundaries where test doubles may be used:

- HTTP ingress: the Battlesnake API transport adapter; contract and integration tests exercise the real in-process application rather than mocking it.
- Runtime diagnostics: the transport adapter may emit bounded, schema-versioned structured events to process stdout/stderr. The domain, search, and evaluation layers return diagnostic data but perform no logging I/O; tests may substitute the declared diagnostics sink or exercise its real serializer.
- Time and deadlines: a `Clock` port.
- Randomness: a `RandomSource` port, used only where the search or evaluation design calls for it (for example move-order jitter or Monte Carlo sampling), never for rule resolution.
- External rule conformance: a test-only `RulesOracle` port, reused from the sibling project's contract where applicable.
- Offline sparring: a `SparringRunner` process port, implemented in this project (not merely declared) since statistically meaningful sparring results are a first-class acceptance concern here.
- Offline reports: a `ReportSink` filesystem port, implemented in this project for the same reason.

The runtime move-decision path has no database, queue, outbound HTTP, or filesystem dependency.

**Rationale.** Narrow boundaries keep the core deterministic, fast, and testable while isolating genuine nondeterminism, external processes, and the sparring/reporting infrastructure this project actually needs.

**Enforcement.** Plans map every external interaction to this list; architecture tests protect dependency direction; review blocks new boundaries without an ADR-backed amendment.

---

### Article VIII - Independent Authorship, Source Provenance, and Architectural Divergence

**Statement.** Every source that materially influences requirements, contracts, algorithms, architecture, or evaluation is recorded before implementation. No code, tests, fixtures, identifiers, module layouts, comments, constants, weights, configuration, or documentation wording is copied, translated, or adapted from Shapeshifter, `snork`, or another unlicensed or unauthorized work. Where this project's design addresses the same sub-problem a consulted reference engine also addresses (for example move search, board representation, or positional evaluation), the resulting data structures, algorithm decomposition, naming, and module boundaries are independently derived and must differ observably from that reference's known shape, not merely carry different identifier names over an equivalent structure.

**Rationale.** Independent authorship and architectural independence are core product constraints for this project specifically, not cosmetic refactoring exercises; the project's purpose is to reach comparable competitive strength through a genuinely different design, not to reproduce one.

**Enforcement.** `PROVENANCE.md` and feature research are reviewed before implementation; each qualifying subsystem's plan or ADR records a short design-diff note against any consulted reference; similarity or provenance concerns block verification until resolved.

---

### Article IX - Exact Rules and Fixed Board Scope

**Statement.** The supported game scope is Standard, non-wrapped, 11x11 boards with two to four active snakes, matching the sibling `rules-core` project's certified scope. Unsupported dimensions, maps, rulesets, and player counts are detected at the transport boundary and are never silently treated as supported.

**Rationale.** A deliberately narrow scope allows greater search depth and evaluation quality without inheriting generic complexity, and keeps this project's supported scope consistent with the rules core it depends on.

**Enforcement.** Contract tests cover supported and unsupported requests; rule-conformance and property tests exercise the complete supported state space by category.

---

### Article X - Inward Dependencies and Strong Domain Types

**Statement.** Domain, search, and evaluation logic do not depend on HTTP, web frameworks, processes, filesystems, or reporting. They may depend on the sibling rules core's public domain types. Domain types make illegal states unrepresentable where practical, and boundary validation rejects remaining invalid inputs.

**Rationale.** Stable dependency direction and explicit invariants support clean design without speculative abstraction, whether the type originates in this project or is reused from the sibling rules core.

**Enforcement.** Architecture tests and compiler checks protect dependency direction; linting forbids ignored errors and unsafe escape hatches unless an ADR documents the need.

---

### Article XI - Controlled Time, Randomness, and Deadline Safety

**Statement.** Domain, search, and evaluation code receive time and randomness through the declared ports. Every search preserves a deterministic legal fallback and stops within its allocated computation budget before the request deadline, returning its best completed result rather than an incomplete one.

**Rationale.** Reproducibility and timely responses are correctness properties in Battlesnake; a multi-turn search that never stops on time is worse than no search at all.

**Enforcement.** Fake-clock and seeded-randomness tests cover cutoff and iterative-deepening behavior; latency tests verify configured safety margins; timeout paths must return the last completed legal result.

---

### Article XII - Domain Coverage and Property-Based Verification

**Statement.** The rule-simulation, search, and evaluation domains each maintain at least 90 percent branch coverage and use property-based tests for state-transition and evaluation invariants in addition to example-based acceptance tests.

**Rationale.** Simulation and search defects are combinatorial and cannot be covered adequately by hand-picked examples alone.

**Enforcement.** CI enforces the coverage threshold; verification requires invariant suites for movement, growth, health, occupancy, elimination, search termination, and evaluation monotonicity properties.

---

### Article XIII - Pinned, Minimal, and Reviewed Dependencies

**Statement.** Runtime dependencies are minimized and locked to reproducible versions. Pre-release dependencies, unmaintained packages, or dependencies with unresolved license or security concerns require an ADR and explicit approval.

**Rationale.** A small, reviewed dependency surface improves reliability, build reproducibility, and provenance clarity.

**Enforcement.** Lockfile review, license and vulnerability auditing, and research freshness checks are required before verification.

---

### Article XIV - Observable Decisions and Measurable Playing Strength

**Statement.** Each move decision exposes structured measurements for elapsed computation, completed search depth, explored nodes, fallback use, and selection rationale without placing external I/O in the decision core. Playing strength is evaluated through reproducible benchmarks and statistically reported sparring against declared opponents, never by subjective inspection alone, and this evaluation is a first-class deliverable of this project rather than a deferred aspiration.

**Rationale.** Performance and strategic improvement require comparable evidence and diagnosable decisions; this is the project's central purpose, not a secondary concern.

**Enforcement.** Feature specifications define measurable latency, search-depth, and strength criteria; benchmark and sparring artifacts record environment, seed, and opponent identity; verification rejects unsupported performance claims.

## Amendments

| Date | Article | Change | ADR |
|------|---------|--------|-----|
