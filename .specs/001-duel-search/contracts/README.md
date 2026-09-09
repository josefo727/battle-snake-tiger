# Contracts - 001-duel-search

## Purpose

Versioned shapes for every boundary touched by feature 001. These artifacts are
the source for contract tests; they are not production implementation files.

## Index

| Contract | Version | Owner | Consumer | Verification |
|----------|---------|-------|----------|--------------|
| `openapi.yaml` | 1.0.0 | This HTTP service owns responses; Battlesnake owns requests | Battlesnake engine and this service | Real Axum router tests plus official request examples |
| `decision-diagnostic.schema.json` | 2.0.0 | Transport diagnostics adapter | Operators, sparring, verification scripts | JSON Schema serialization tests |
| `rules-core-dependency.rs` | 1.0.0 | `rules-core` at commit `efed780` | Engine transport, fallback, and differential tests | Compile-time contract test importing every listed item |
| `sparring-report.schema.json` | 1.0.0 | `tiger-sparring` report sink | Verification and later features | JSON Schema serialization tests |
| `opponent-roster.yaml` | 1.0.0 | Third-party opponent projects | Offline sparring runner | Provisioning and liveness checks before any game |

## Conventions

- Breaking shape changes bump the contract major version.
- Inbound Battlesnake objects accept unknown members for forward compatibility.
- Outbound webhook responses reject additional members in tests.
- The sparring subsystem, opponents, and the rules CLI exist only in the offline
  environment; none is included in the runtime binary or image.
- The diagnostics schema extends the sibling's v1 schema with search fields; v1
  events are not emitted by this service.

## Sources

- `../research.md` sections **Battlesnake API and Standard rules**, **rules-core rules core**, **snork agents**, **Web and runtime crates**
