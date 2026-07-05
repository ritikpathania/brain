# ADR-014: Deterministic Execution

## Status
Accepted

## Context
Non-determinism makes system behavior hard to reproduce, troubleshoot, and evaluate. Standard system times are mutable and depend on hardware. Standard hashers (like `DefaultHasher` in Rust) are not stable across compiler versions or runs, which would break sticky session A/B routing.

## Decision
We enforce absolute determinism throughout serving and routing pipelines:
1. Systems must not retrieve system time internally. Instead, time must be resolved through a mockable `Clock` interface passed via context.
2. Hashing for sticky session routing must use a stable, explicit algorithm (64-bit `FNV-1a`) that produces identical values regardless of runtime environment, compiler version, or hardware layout.
3. Fallback strategies (such as missing session IDs) must yield deterministic defaults (routing to the baseline variant) rather than falling back to non-deterministic random splits.

## Alternatives Considered
* **Standard Library Hasher**: Using `std::collections::hash_map::DefaultHasher`. Rejected because it does not guarantee stability across platforms or compiler version upgrades.
* **Random Splitting**: Falling back to random number generators. Rejected because it destroys consistency across query evaluations.

## Related ADRs
* [ADR-011 (Immutable Snapshots)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-011-immutable-snapshots.md)
* [ADR-013 (Behavioral Invariants)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-013-behavioral-invariants.md)

## Expected Stability
Long-term.
