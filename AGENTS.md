# Brain engineering instructions

Brain uses the Full Production layout from the workspace engineering playbook. The pinned stack is Rust 1.99.1, Bun 1.4.4, and uv-managed CPython 3.15.2. [Engineering guidelines](GUIDELINES.md) record the package-specific TypeScript pins and implementation rules.

| Rule | Where to verify |
| --- | --- |
| Wire features through the live runtime composition. Keep `Dummy*` and `Noop*` fixtures out of production paths. Keep domain entities free of database, async, logging and FFI dependencies. Route storage through repository interfaces. | [Architecture](ARCHITECTURE.md) and [test matrix](TEST_INFRA.md) |
| Domain events represent facts. Translate them to persistence envelopes and UI stream frames at the application/service boundary. Preserve versioned DTOs and the frontend's authoritative session, monotonic streams, exact-once settlement, no-replay recovery and allowlisted context metadata. | [Protocol](docs/reference/protocol.md) and [frontend](docs/subsystems/tui.md) |
| Keep Fleet registration, liveness and preferences in the daemon. Use stable IDs and requester-relative snapshots. Keep foreground registrations and background jobs distinct from durable conversations. | [Frontend handbook](docs/subsystems/tui.md) |
| Preserve frozen presentation and reference fixtures unless a change is specifically authorized. Verify a zero diff or recorded hash for affected baselines. Keep private oracles and vendor references out of source and distribution. | [Guidelines](GUIDELINES.md) and [invariant ledger](REGRESSION-PREVENTION-AND-INVARIANTS.md) |
| Start with existing models and interfaces. Add foundational abstractions only when reuse is insufficient, at least two features share the need, and the abstraction supports future capabilities. | [Architecture](ARCHITECTURE.md) |

## Phase workflow

1. Start with the feature and interface inventory in [PROJECT.md](PROJECT.md) and acceptance criteria in [ROADMAP.md](ROADMAP.md). For a multi-step phase, use the Superpowers planning and EGM draft skills to write `docs/designs/phase-N-<topic>.md` with files, contracts, transitions, failure cases, and acceptance checks. Have a separate reviewer run EGM review before implementation.
2. Record milestone dependencies and exclusive file ownership in PROJECT.md before dispatching subagents. Use Superpowers test-driven development and subagent-driven development for scoped implementation. Map each changed capability to the applicable tiers in [TEST_INFRA.md](TEST_INFRA.md) and record readiness and gaps in [TEST_READY.md](TEST_READY.md).
3. Cross-check the implementation independently against the phase spec and frozen boundaries. Run the relevant build, lint, and offline tests. Record drift in a dated report under `docs/designs/` and link it from the docs index while it informs active work. Keep completed reports in Git history once their lasting decisions have a canonical home.
4. Update the current docs when a command, contract or milestone changes. Keep user work and preservation evidence intact. Refresh the code graph, clean phase scratch output, and commit the scoped result.

## Diagnosis and verification

- Start code exploration with `code-review-graph` when available. Query changes, callers, affected flows, and tests, then check exact files. Refresh with `CRG_PARSE_EXECUTOR=thread code-review-graph update`.
- Before fixing a defect, read [DEAD_ENDS.md](DEAD_ENDS.md) and the [invariant ledger](REGRESSION-PREVENTION-AND-INVARIANTS.md). Extract timing deltas from the failing run and trace the exact platform, startup mode and execution path. Record the symptom, root cause, enforced invariant and maintained check for each fix. Record abandoned attempts in DEAD_ENDS.md.
- Run `cargo xtask verify` for implementation changes. For first-party frontend changes, run typecheck, the owned suite and `scripts/verify-frontend-e2e.sh` with explicit binaries. Measure draw latency and memory for presentation changes. Run `cargo xtask docs all` for documentation-only work. Follow the [release checklist](docs/guides/release-checklist.md) for native archive acceptance.
