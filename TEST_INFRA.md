# Brain test infrastructure

The suites live with their owning code. Each phase maps changed behavior to these tiers and records any uncovered cases in [TEST_READY.md](TEST_READY.md). Aim for five meaningful cases per applicable feature and boundary tier when the behavior warrants them. Run with local fixtures, local sockets, or in-process dependencies. Scope and isolate environment mutations, and give tests their own temporary state.

## Five-tier workspace suites

| Tier | Coverage | Maintained entry points |
| --- | --- | --- |
| 1. Features | Runtime startup, provider wiring, extraction and embedding contracts | [Feature tests](crates/brain-services/tests/e2e_tier1_features.rs) |
| 2. Boundaries | Empty and oversized data, Unicode, malformed JSON, provider faults | [Boundary tests](crates/brain-services/tests/e2e_tier2_boundaries.rs) |
| 3. Combinations | Extractor to storage, session cache to persistence, storage to retrieval | [Combination tests](crates/brain-services/tests/e2e_tier3_combinations.rs) |
| 4. Scenarios | Multi-turn isolation, reflection, daemon recovery | [Scenario tests](crates/brain-services/tests/e2e_tier4_scenarios.rs), [daemon lifecycle](daemon/tests/daemon_lifecycle_tests.rs), [terminal PTY](packages/brain-frontend/tests/e2e/terminalPty.test.ts) |
| 5. Adversarial | Runtime races, lock lifetimes, cancellation, provider fault handling | [Runtime stress](crates/brain-services/tests/adversarial_tier5_runtime.rs), [provider stress](crates/brain-services/tests/adversarial_tier5_extraction_provider.rs), [stream stress](daemon/tests/uds_generation_adversarial_tests.rs) |

## Coverage by maintained contract

| Contract from PROJECT.md | Maintained checks |
| --- | --- |
| Runtime, compiler, SQLite and retrieval | Workspace tiers, crate-local unit/integration tests and [ranking baselines](docs/reference/retrieval-baselines/README.md). |
| Daemon protocol, sessions and permissions | [Frontend session contract](daemon/tests/frontend_session_contract_tests.rs), [generation lifecycle](daemon/tests/uds_generation_lifecycle_tests.rs), [permission round trip](daemon/tests/uds_permission_roundtrip_tests.rs), and [frontend tests](packages/brain-frontend/tests). |
| SDKs and protocol adapters | Rust workspace and [contract generation](docs/reference/generation_workflow.md), [Python SDK tests](sdks/python/tests), and TypeScript SDK `bun run test`. |
| Product delivery | [Portable daemon runtime tests](daemon/tests/portable_runtime_tests.rs), [manifest and smoke tests](scripts/release), and the [native release checklist](docs/guides/release-checklist.md). |
| Architecture and toolchain | [Dependency rules](crates/brain-arch-tests/tests/dependency_boundaries.rs) check Cargo dependency direction. [Fitness tests](crates/brain-fitness-tests/src/lib.rs) check domain dependencies, adapter storage isolation and PyO3 placement. The [toolchain suite](crates/brain-fitness-tests/tests/toolchain_contract_tests.rs) checks the pins and installation docs. |

Run `cargo xtask verify` for formatting, Clippy, workspace tests, contracts, architecture and docs. The [guidelines](GUIDELINES.md) cover frontend and native verification. A local socket denial is a sandbox limitation to resolve before reporting the test as passed.
