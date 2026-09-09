# ADR 0002 - Reuse the sibling rules core as oracle and fallback; add a compact search kernel

- **Status:** accepted
- **Date:** 2026-09-09
- **Deciders:** José R. Gutierrez
- **Context links:** `../001-duel-search/spec.md` criteria 1, 2, 5; `../001-duel-search/research.md` §rules-core rules core; Constitution Articles IV, VIII, X, XII

## Context

The project reuses the independently authored, conformance-verified
`rules-core` rules core (approved during planning intake). Its
`resolve_turn` is exact but allocates (`String` ids, `Vec` bodies) on every call
and measured about 0.42 us per transition. A search that must reach real depth
within roughly 380 ms needs to evaluate millions of positions; its inner loop
cannot allocate.

## Options considered

### Option A - Call `resolve_turn` from inside the search

- Pros: zero new rules code; exactness inherited for free.
- Cons: about 2.4 million transitions/second single-threaded caps the tree at
  roughly one million nodes per move, which limits depth to a few plies
  even with good ordering.
- Effort / risk: minimal effort, structurally limits playing strength.

### Option B - Compact allocation-free kernel, differentially tested against `resolve_turn`

- Pros: an order-of-magnitude higher node rate is plausible with a fixed-size
  `Copy` position and `u128` cell sets; exactness is not re-derived from scratch
  but proven equal to an already-certified reference on generated states.
- Cons: a second implementation of the turn rules exists in this repository and
  must never diverge from the reference.
- Effort / risk: moderate; the differential property suite is the safeguard.

### Option C - Rewrite the whole rules layer independently, sharing nothing

- Pros: maximal architectural independence.
- Cons: repeats T004-T007, T018, T021 of the sibling for the same official rules
  and discards its conformance evidence.
- Effort / risk: highest effort, no benefit for a shared public specification.

## Decision

Option B. `tiger-engine` depends on `rules-core` (path dependency,
commit `efed780` recorded in `contracts/rules-core-dependency.rs`) for: wire DTOs
and the supported-scope predicate, `TurnState` and `resolve_turn` as the
reference model, the `Clock` port, and the one-turn safety fallback. The search
kernel (`arena`) is new code with its own types. Design-diff note: the
publicly documented shape of the reference engines is a general-purpose engine
that also handles other sizes and modes; this kernel is deliberately fixed to
exactly two snakes on 11x11, so it needs no generic parameters and its position
type is a flat `Copy` value.

## Consequences

- **Positive:** a high node rate without giving up certified correctness;
  fallback behavior for non-duel requests is literally the certified code.
- **Negative:** two rules implementations to keep equal; the sibling path
  dependency ties this workspace to a sibling checkout at a known commit.
- **Follow-ups:** T-level task for the differential property suite must precede
  any search task; a benchmark task records kernel vs. reference throughput.

## Constitution impact

None. Article IV is satisfied by `contracts/rules-core-dependency.rs`; Article X
by keeping the kernel free of transport, process, and filesystem types.

## References

- Research entries: `../001-duel-search/research.md` §rules-core rules core
