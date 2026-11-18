# Brain engineering guidelines

## Toolchains and provenance

CI and `rust-toolchain.toml` select Rust 1.99.1. The maintainer host currently maps its `1.99.1-aarch64-apple-darwin` toolchain directory to `stable-aarch64-apple-darwin`, which reports Rust and Cargo 1.100.0. Treat the requested toolchain name and actual command versions as separate facts. Product manifests record the actual assembly commands. Preserve those versions and exact source/artifact identities in verification evidence. Use Bun 1.4.4, TypeScript 5.9.3 and the uv-managed CPython 3.15.2 selected by the frozen daemon project.

## Runtime and dependency boundaries

| Category | DO (Mandatory Pattern) | DO NOT (Forbidden Anti-Pattern) |
| :--- | :--- | :--- |
| **Domain Modeling (`brain-domain`)** | Keep `brain-domain` pure and enforce invariants inside entity/aggregate methods and return pure `DomainEvent` values. | Import `tokio`, `rusqlite`, `tracing`, `reqwest`, or `pyo3` in `brain-domain`, or put business validation in services. |
| **Concurrency & Locks** | Use `parking_lot::{Mutex, RwLock}` for synchronous locks. Clone `Arc`s and drop read/write guards before calling `block_in_place` or `.await`. | Use `std::sync::Mutex::lock().unwrap()` (poison panics) or hold lock guards across blocking/async bridges. |
| **Error Handling** | Return typed `Result<T, BrainError>` / `ApplicationError` (`thiserror`) and propagate startup/scheduler errors with `?`. | Discard startup errors with `let _ = ...`, `.unwrap()` in library crates, or return silent `[0.0; 384]` dummy vectors on provider failure. |
| **Cargo Dependencies** | Keep `test-utils` features in dev dependencies. Declare embedded Python support through the feature that enables it and honor the package build contract. | Leak `features = ["test-utils"]` into runtime dependencies or silently change the packaged Python runtime contract. |
| **Process & Tool Execution** | Spawn owned Unix process groups, drain pipes, escalate `SIGTERM` -> `SIGKILL` on timeout/cancel, and reap before settling. | Use fire-and-forget `spawn_blocking(Command::output())` that leaves orphaned background processes on cancellation. |

## Frontend boundaries

The maintained frontend is `packages/brain-frontend`. Adapt protocol and state under the existing first-party presentation contracts. Frozen visual components, keybindings and oracle fixtures require explicit authorization to change. The retired shell and manual increment PTY scripts remain outside the tracked product tree. See the [Phase 6 acceptance plan](docs/designs/phase-6-legacy-frontend-retirement.md).

Use the typed adapter and state layers for handshake, session hydration, stream correlation and reconnect. Preserve the allowlisted `metadata.brain_context` wire shape and the count-only context view. Fleet preferences and requester-relative snapshots belong to the backend contract.

## Offline test coverage

Use representative feature tests, boundary and fault cases, cross-module combinations, multi-turn or restart scenarios, and concurrency or property tests where the affected feature calls for them. Prefer in-process fixtures and isolated temporary directories. Scope environment mutations with a drop guard and serialize tests that share the affected variables. Make crash checkpoints deterministic with pacing and an observed non-terminal frame.

## Verification

Run `cargo xtask verify`, frontend typecheck and owned tests, and the real-daemon E2E gate. Run the native archive workflow for the exact release candidate on Linux x86_64 and macOS arm64. Verify the extracted closed product and record source SHA, target, actual tool versions, job URL and archive SHA-256. Preserve unrelated local working files and historical evidence.
