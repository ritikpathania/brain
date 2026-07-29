# Foundational Architectural Invariants

This document establishes the 6 core architectural invariants governing the `brain` workspace runtime, host layer, and subsystem boundaries.

These invariants are enforced by automated AST architecture tests in `crates/brain-arch-tests`. Any pull request or refactoring that violates these rules will fail CI verification.

---

## Architectural Governance & ADR Policy

> **Rule**: Any pull request that changes, removes, or weakens an architectural invariant must include an **Architecture Decision Record (ADR)** explaining the motivation, alternatives considered, migration strategy, and impact on existing architecture tests.

- **Implementation Changes**: Follow standard implementation plans and pass workspace tests (`cargo test --workspace`).
- **Architectural Invariant Changes**: Require an approved ADR in `docs/architecture/adr/` and an explicit update to this document (`ARCHITECTURE_INVARIANTS.md`).

---

## The 6 Core Architectural Invariants

### Invariant 1: Orchestration Only
> `ApplicationRuntime` owns **orchestration only**.

- **Rule**: Storage queries, SQL statements, and domain business logic must never exist directly inside `ApplicationRuntime` or `ApplicationFacade`.
- **Enforcement**: Subsystem delegation only; no direct storage handles in runtime facade.

### Invariant 2: Host OS Isolation
> Hosts own all **OS integration**.

- **Rule**: Operating system concerns (PID file management, UNIX Domain Socket binding, HTTP REST servers, process signal traps like `SIGTERM`/`SIGINT`) belong strictly inside host executables (`DaemonHost` in `daemon/` and `CLIHost` in `apps/brain`).
- **Enforcement**: Crate-level AST test asserting runtime/service crates never import `libc`, `tokio::net::UnixListener`, or `std::process::exit`.

### Invariant 3: Transport Independence
> Subsystems **never depend on transport**.

- **Rule**: Core domain models and runtime services must be completely protocol-independent.
- **Enforcement**: Zero imports of UDS framing, HTTP request headers, or CLI flags within `brain-domain`, `brain-core`, and `brain-services`.

### Invariant 4: Single Runtime Graph
> There is **exactly one runtime graph**.

- **Rule**: Embedded mode, CLI mode, and daemon mode execute against the exact same runtime graph topology.
- **Enforcement**: Zero feature divergence between `CLIHost` (Embedded Transport) and `DaemonHost` (UDS Transport).

### Invariant 5: Builder Instantiation Only
> All entry points instantiate the runtime via **`RuntimeBuilder`**.

- **Rule**: `RuntimeBuilder` is the sole entry point for constructing `ApplicationRuntime`.
- **Enforcement**: Executables must invoke `RuntimeBuilder::build()` as their only runtime creation path; ad-hoc manual service instantiation is forbidden.

### Invariant 6: Single Composition Root
> `ApplicationRuntime` is the **only composition root**. Subsystems must never construct or own sibling subsystems.

- **Rule**: Subsystems receive their dependencies via injection from `RuntimeBuilder`. Subsystems must never instantiate sibling subsystem types (e.g. `PluginManager` instantiating `RetrievalService`).
- **Enforcement**: Subsystem constructors accept dependencies as parameters; internal instantiation of sibling types is forbidden.

---

---

## Behavioral Architecture Testing Roadmap

In addition to static AST dependency boundary checks (`crates/brain-arch-tests/tests/dependency_boundaries.rs`), the architecture is verified via behavioral integration tests that assert runtime guarantees at execution time:

1. **Host Instantiation Parity**: Assert that `DaemonHost` and `CLIHost` produce identical `ApplicationRuntime` initialization topology and service behavior.
2. **Phase Execution Determinism**: Assert that startup and shutdown phases execute in exact, deterministic ordering across all host modes.
3. **Sole Construction Entry Point**: Assert that `RuntimeBuilder` remains the exclusive runtime construction path without bypass.
4. **Isolated Service Instantiability**: Assert that transport-independent services (`BrainEngine`, `RetrievalService`, `SessionManager`) can be instantiated cleanly in unit/integration test isolation without launching host network listeners or daemon infrastructure.

