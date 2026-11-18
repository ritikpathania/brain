# Regression Prevention and Invariants

## Core System & Runtime Invariants

1. **Zero External Dependencies in `brain-domain`**:
   - `crates/brain-domain` must stay at the bottom of the dependency DAG with zero async runtime (`tokio`), database (`rusqlite`), logging (`tracing`), or FFI (`pyo3`) dependencies.
   - Domain mutations (`Edge::strengthen`, `Session::archive`, etc.) must validate their own business invariants and emit pure, side-effect-free `DomainEvent` values.
2. **Strict Event Layer Separation**:
   - `DomainEvent` (immutable business fact), `EventEnvelope` (persistence/transport metadata), and `StreamEvent` (TUI/UDS streaming chunk) must remain distinct types. `crates/brain-services` and `crates/brain-application` act as the sole translation boundary.
3. **18-Step / 6-Phase Knowledge Compiler Purity**:
   - No background worker or transport handler may directly mutate canonical knowledge graph topology in SQLite outside the deterministic compilation and reflection command boundaries (`PassManager::default_pipeline()` in `crates/brain-services/src/compiler/`).
   - Compilation must remain 100% idempotent ($P(P(G)) = P(G)$) with deterministic tie-breaking (highest confidence -> most recent provenance timestamp -> lexicographically smallest ID).
4. **Reflection Analysis vs. Approved Mutation Separation**:
   - Reflection passes evaluate bounded, read-only snapshots and emit `ReflectionFinding` records. Only `ReflectionPlanner` / `ReflectionCommandHandler` (or explicitly approved `KnowledgeEvolutionPlan` executions) may apply mutations inside a storage transaction.
5. **Single Authoritative Runtime Topology**:
   - `apps/brain` (`brain daemon ...`) delegates daemon lifecycle operations to `brain-daemon`. `DaemonHost::run_server` constructs a single `BrainRuntime` instance shared with `ApplicationRuntime` and `BrainApplication`.
6. **First-party frontend and Fleet ownership**
   - Keep typed protocol and state adaptation below the maintained first-party presentation layer. Preserve retired visual files and oracle receipts as historical inputs.
   - The daemon owns Fleet identity, liveness and durable preferences. Snapshots derive requester identity and expose foreground registrations and background jobs. First-party UI acceptance follows the maintained frontend contract.

## Numbered bug and regression log

### 1. Clean Checkout Could Not Start the First-Party Frontend

- **Symptom** — The frontend entry point was missing from a Git export, and the native launcher still selected the legacy shell.
- **Root cause** — The package, launcher, and daemon integration were split between committed and working-tree files.
- **Invariant** — The first-party package and its protocol tests are tracked. The CLI launches that package and the daemon negotiates its versioned handshake, sessions, and generation stream.
- **Regression test** — Run `bun run build` and the real-daemon frontend E2E suite from a clean checkout. The E2E suite includes `brain ui` from an unrelated working directory.

### 2. Live Generation Missed Scoped Constraints and Vector Matches

- **Symptom** — Memory without lexical overlap was absent from generated context.
- **Root cause** — The context orchestrator discovered storage candidates from full-text hits and their graph neighbors only.
- **Invariant** — Context applies scope and deletion checks, recalls vector matches from the configured provider, and prioritizes persisted constraints within the existing budgets.
- **Regression test** — Run `cargo test --locked -p brain-services --test context_orchestrator_tests` and `cargo test --locked -p brain-daemon --test uds_retrieval_augmentation_tests`.

### 3. Failed Context Assembly Looked Like an Empty Result

- **Symptom** — A retrieval error appeared as successful zero-result context and cleared the last successful frontend summary.
- **Root cause** — The daemon fabricated a fallback snapshot and the frontend conflated absent and empty snapshots.
- **Invariant** — Context metadata is present only when assembly produced a snapshot. An explicit successful empty snapshot clears the previous summary. Cancellation settles without waiting for semantic lookup.
- **Regression test** — The UDS retrieval suite exercises a broken retrieval backing store. Frontend context state tests cover absent and empty snapshots.

### 4. Shell Acknowledgement Raced Transcript Persistence

- **Symptom** — A client loading a session immediately after standalone shell execution could observe only the submitted command.
- **Root cause** — The daemon flushed the completion response before persisting the tool outcome.
- **Invariant** — The transcript update is attempted before sending the completion acknowledgement. The frontend projects both historical `executed` envelopes and current typed outcomes.
- **Regression test** — Run `cargo test --locked -p brain-daemon --test uds_feedback_loop_tests` and `bun test tests/state/interactions.test.ts`.

### 5. Release Builds Omitted the Real-daemon Frontend Gate

- **Symptom** — A tag build or local product assembly could proceed after frontend unit tests alone.
- **Root cause** — The required real-daemon and actual-launcher E2E command existed only in branch and pull-request CI.
- **Invariant** — CI, native platform release builds, and local packaging all run the same E2E gate against explicit Brain CLI and daemon binaries. Failed, skipped, or pending cases fail the gate before standalone compilation.
- **Regression test** — Run the frontend CI and release workflow contract tests, then the native package orchestrator.

### 6. Generation Stress Tests Expected Obsolete Start Metadata

- **Symptom** — The stress suite failed against the first-party frontend contract because it expected internal telemetry in the generation start frame.
- **Root cause** — The fixture predates the allowlisted context projection. The crash-checkpoint fixture also used an unpaced mock and could observe a turn that had already completed.
- **Invariant** — The tests check the public context projection at start, terminal telemetry at stream end, and a paced thinking checkpoint before killing the isolated test daemon.
- **Regression test** — Run `cargo test --locked -p brain-daemon --test uds_load_stress_tests`.

### 7. Evaluation Tests Overwrote Tracked Release Inputs

- **Symptom** — Workspace tests could turn a clean package source dirty by rewriting tracked ranking reports.
- **Root cause** — Two evaluation tests wrote generated Markdown directly into their checked-in evidence directory.
- **Invariant** — Tests render reports into test-owned temporary directories. Historical evaluation reports remain stable release inputs.
- **Regression test** — Run the cross-validation and LambdaMART suites and compare `git diff -- crates/brain-services/tests/evaluation` before and after.

### 8. Native Workflow Created the Product Output Too Early

- **Symptom** — Native Linux verified both binaries and the standalone frontend, then stopped before creating the archive.
- **Root cause** — The workflow created the exact directory that the product assembler requires to be absent when it starts.
- **Invariant** — The workflow creates a unique parent, gives the assembler an unused child named `brain-dist`, and passes that same layout to relocation verification and archiving.
- **Regression test** — The release workflow integration test executes the assembly, relocation, and archive shell steps with an assembler fixture that rejects an existing output directory.

### 9. Hosted CPython Contained Build-Host Paths

- **Symptom** — Linux assembly rejected the staged daemon runtime for containing a developer-home path. The parallel macOS job was cancelled before assembly.
- **Root cause** — The hosted CPython distribution shipped its source checkout path in ELF debug metadata and generated sysconfig. Its copy of the IDLE test suite also contained sample home paths.
- **Invariant** — Both native jobs compile and package against the same uv-managed standalone CPython pinned by the daemon project. Only the pre-package E2E step gets the build interpreter's loader path. Native archive checks stay blocking, and matrix jobs finish independently so each target has its own result.
- **Regression test** — Release workflow tests verify the pinned interpreter selection, the native build and package consumers, and the matrix policy. The full native workflow runs host-path and relocation checks on the assembled archive.

### 10. Mach-O Scanner Rejected Its Own Staging Path

- **Symptom** — The hosted macOS runtime verifier rejected a staged daemon under `/Users/runner` despite its relocated dependency list.
- **Root cause** — `otool -L` prints the inspected filename as a header. The scan treated the header as a dependency.
- **Invariant** — Inspect only the dependency lines for embedded host paths. Continue to enforce the packaged Python install name and the artifact-relative rpath.
- **Regression test** — The runtime manifest suite runs the verifier with an `otool` fixture that places a home path in the header and separately in a dependency.

### 11. Retired Shell Cannot Re-enter the Product Checkout

- **Symptom** — The first-party launch and packaging path was independent of the legacy shell, but historical launch scripts and the package still appeared as available entry points.
- **Root cause** — The old package and manual harnesses remained tracked after first-party launch became authoritative.
- **Invariant** — Current product checkouts omit the retired shell and increment PTY launch scripts. The no-fallback launcher contract stays enforced and historical files remain recoverable at the pre-retirement commit.
- **Regression test** — The canonical frontend CI test checks tracked paths for the retired package and entry points. Real-daemon E2E exercises actual `brain ui` with the first-party frontend.

### 12. Ordinary CI Consumed the Retired Flat Archive Layout

- **Symptom** — The SDK packaging job omitted Bun and attempted to launch extracted binaries outside the new `brain-dist/bin` tree.
- **Root cause** — That consumer retained the daemon-only archive assumptions after the complete product layout became authoritative. Its early-exit inventory pipelines could also reject large valid archives under `pipefail`.
- **Invariant** — The job provisions pinned build prerequisites, assembles with the managed standalone Python, validates all required product paths without a broken writer pipe, and executes binaries from an owned extraction directory. Failures propagate and temporary extraction is cleaned up.
- **Regression test** — The frontend CI suite executes the workflow's packaging step under `bash -e -o pipefail` with real large and incomplete fixture archives and failing package/binary commands.

### 13. Release Acceptance Checklist Described Unsupported Frontend Actions

- **Symptom** — An active release checklist required obsolete sidebar behavior and then included session/Fleet controls absent from the first-party frontend.
- **Root cause** — Acceptance requirements were copied from older frontend and backend capabilities without checking the maintained adapter and key router.
- **Invariant** — The checklist follows the current create/list/load session contract, Ctrl+O picker, focus-specific Escape behavior, bounded count-only context panel and no-replay recovery. Toolchain provenance distinguishes the selected local toolchain alias from actual compiler versions.
- **Regression test** — Independently inspect the checklist against `keyRouter.ts`, `sessions.ts`, `interactions.ts`, current context rendering and the existing frontend suites. Run `cargo xtask docs all` for links and document metadata.

### 14. Ordinary CI Invoked an Undeclared Maturin Build

- **Symptom** — The parent workspace gate and SDK job attempted `uv run maturin develop`, which failed before the packaging consumer could run.
- **Root cause** — The daemon's frozen environment declares no Maturin dependency or Python extension build project. The SDK uses the pure Python `brain_sdk` package, and Rust/PyO3 verification belongs to Cargo.
- **Invariant** — Ordinary CI syncs the declared environment and retains the Cargo/PyO3 workspace gate and Python SDK tests without invoking an unavailable extension builder.
- **Regression test** — The focused workflow test rejects Maturin invocation in the parent gate while requiring `cargo xtask verify`. Run the SDK suite with local UDS binds enabled.

### 15. Pre-Package Linux Frontend E2E Lacked the Build Interpreter Loader Path

- **Symptom** — Ordinary Linux packaging selected the standalone Python, then launched the unrelocated daemon before its Python library could be resolved.
- **Root cause** — The local orchestrator supplied a macOS framework path only. The native release workflow already supplied Python library search paths for its pre-package E2E stage, and Linux rpath relocation happens later during assembly.
- **Invariant** — Only the pre-package E2E command receives the managed interpreter library directory in `LD_LIBRARY_PATH` and `DYLD_FALLBACK_LIBRARY_PATH`, retaining inherited entries. Product assembly and extraction verification receive no newly injected library search override.
- **Regression test** — Execute the package orchestrator with controlled build stages and verify child environments at the E2E, runtime-verification and product-extraction boundaries.

### 16. Borrowed SQLite Transaction Thread-Safety (`Send`/`Sync` Soundness)
- **Symptom**: Potential undefined behavior and concurrent connection corruption if a borrowed SQLite transaction facade was shared across threads.
- **Exact Root Cause**: Borrowed `rusqlite` transaction wrappers previously had `unsafe impl Send` / `unsafe impl Sync` assertions even though the underlying SQLite connection/transaction is not thread-safe.
- **Code Invariant Enforced**: Removed `unsafe impl Send/Sync` from borrowed transaction facades in `crates/brain-storage`; only the owned `r2d2` connection pool (`SqliteStorage`) is `Send + Sync`.
- **Regression Test**: `cargo test -p brain-storage` and `cargo xtask architecture-check`.

### 17. Python HostContext Use-After-Free Across FFI Boundary
- **Symptom**: Retained Python plugin objects could dereference a dangling `HostContext` pointer after the Rust call scope exited.
- **Exact Root Cause**: The PyO3 bridge previously transmuted a borrowed `&dyn HostContext` lifetime to `'static` inside a Python wrapper object.
- **Code Invariant Enforced**: Replaced raw pointer transmutation with an `Arc`-backed runtime bridge inside `crates/brain-python` that explicitly closes after hook execution; subsequent Python calls on retained references fail deterministically with `"RuntimeContext is closed"`.
- **Regression Test**: `cargo test -p brain-python --test python_tests`.

### 18. Orphaned Child Processes on Bash Tool Timeout or Cancellation
- **Symptom**: Cancelling or timing out a shell tool invocation returned an error to the caller while the spawned OS shell process and its child processes continued running in the background.
- **Exact Root Cause**: `spawn_blocking(Command::output())` could not interrupt or reap an in-flight child process group when the awaiting Tokio future was dropped or timed out.
- **Code Invariant Enforced**: `daemon/src/tools/bash_tool.rs` spawns an owned child process group on Unix, drains stdout/stderr pipes, sends `SIGTERM` to the process group on cancellation/timeout, escalates to `SIGKILL` after a bounded grace window, and reaps the child before settling the tool result.
- **Regression Test**: `cargo test -p brain-daemon --test uds_tool_execution_tests`.

### 19. Incomplete Session Hydration Missing `system_prompt` (SQLite v17 Seam)
- **Symptom**: Reloading a persisted session dropped its custom `system_prompt`, causing resumed conversations to revert to default instructions.
- **Exact Root Cause**: `load_session_conn` in `crates/brain-storage/src/store.rs` selected session metadata without hydrating both `history` and `system_prompt` from their authoritative SQLite v17 columns.
- **Code Invariant Enforced**: `load_session_conn` explicitly selects `SELECT history, system_prompt FROM sessions` and hydrates the `Session` aggregate from authoritative columns.
- **Regression Test**: `cargo test -p brain-storage --test storage_tests` and `daemon/tests/uds_product_integration_tests.rs`.

### 20. Pre-Registration Generation Cancellation Race (No-Op Cancel)
- **Symptom**: Pressing cancel immediately after submitting a prompt was acknowledged by the daemon as a no-op, allowing the generation stream to run to completion anyway.
- **Exact Root Cause**: `v1/generation/cancel` arrived at the daemon/adapter before the generation stream registration completed, so the cancel handler found no active stream and dropped the request.
- **Code Invariant Enforced**: The generation controller retains a pending cancellation intent until a validated `stream_start` frame is observed, then immediately dispatches cancellation (or abandons the intent if stream startup fails).
- **Regression Test**: `daemon/tests/uds_generation_lifecycle_tests.rs` and `packages/brain-frontend/tests/state/interactions.test.ts`.

### 21. Unbounded Context Metadata Growth & Raw Score Leakage Over UDS
- **Symptom**: Long-running frontend sessions accumulated unbounded `(session_id, generation_id)` context entries in memory, and internal ranking scores/telemetry leaked into UDS stream frames.
- **Exact Root Cause**: The UDS generation stream serialized internal retrieval diagnostics directly and stored every completed generation's context metadata indefinitely.
- **Code Invariant Enforced**: Restricted external stream metadata to the allowlisted `metadata.brain_context` schema (`version`, `retrieval_epoch_id`, `retrieval_count`, `retrieval_channels`, `context_token_count`, `source_refs`) and bounded adapter retention to the single active generation plus the latest completed summary per session.
- **Regression Test**: `daemon/tests/uds_retrieval_augmentation_tests.rs`.

### 22. Permissive Session ID Hashing Masking Invalid ULIDs
- **Symptom**: Malformed or arbitrary string session IDs were silently hashed into synthetic IDs instead of returning validation errors, hiding client bugs and risking cross-session collisions.
- **Exact Root Cause**: Legacy UDS session handlers normalized arbitrary strings into fallback hashes when ULID parsing failed.
- **Code Invariant Enforced**: Canonical session endpoints require exact Brain-issued ULID strings (`SessionId::from_str`), returning distinct typed errors for missing, malformed, and not-found session IDs.
- **Regression Test**: `daemon/tests/uds_session_mutation_tests.rs`.

### 23. Reconnect Auto-Replay Duplicating Non-Idempotent Turns & Tool Calls
- **Symptom**: Transient UDS disconnects during an active generation or permission prompt could re-send the user turn or re-execute a tool upon reconnect.
- **Exact Root Cause**: Early shell reconnect logic queued outbound requests in a FIFO buffer and replayed them automatically after socket reconnection.
- **Code Invariant Enforced**: Reconnect performs a fresh `v1/frontend/handshake`, revalidates the `SessionId`, settles any interrupted generation once, and never replays non-idempotent turns, tool executions, or permission decisions. Protocol incompatibility and `SessionNotFoundError` stop recovery immediately without retry.
- **Regression Test**: `packages/brain-frontend/src/brain/recovery.ts` unit tests and `daemon/tests/uds_generation_adversarial_tests.rs`.

### 24. Production `test-utils` Feature Leakage in `brain-services`
- **Symptom**: Production builds of `brain-services` compiled test-only storage helpers from `brain-storage`.
- **Exact Root Cause**: `crates/brain-services/Cargo.toml` listed `brain-storage = { ..., features = ["test-utils"] }` under normal `[dependencies]` instead of restricting it to `[dev-dependencies]`, and `ProductionCorpusBuilder::build()` called a test-only constructor.
- **Code Invariant Enforced**: Removed `features = ["test-utils"]` from `[dependencies]` in `crates/brain-services/Cargo.toml` (keeping it strictly in `[dev-dependencies]`) and refactored `ProductionCorpusBuilder::build()` in `crates/brain-services/src/retrieval/eval_harness/production_corpus.rs` to use `SqliteStorage::new`.
- **Regression Test**: `crates/brain-services/tests/e2e_tier1_features.rs` and `cargo tree -p brain-services --edges no-dev`.

### 25. Double `ReflectionEngine` Initialization & Ignored Scheduler Startup Errors
- **Symptom**: `BrainRuntime::new` instantiated two separate reflection engines and silently ignored startup failures from background schedulers.
- **Exact Root Cause**: `crates/brain-services/src/brain_runtime.rs` shadowed `reflection_engine` with a second instance and used `let _ = reflection_scheduler.start();` and `let _ = runtime_orchestrator.start();`.
- **Code Invariant Enforced**: Disambiguated `sqlite_reflection_engine` (used by `SqliteCanonicalizer`) from `reflection_engine` (used by the background scheduler) and propagated `.start()?` errors via `Result`.
- **Regression Test**: `crates/brain-services/tests/adversarial_m1_audit_tests.rs`.

### 26. `std::sync::Mutex` Thread-Poisoning Panics in Runtime Diagnostics & State
- **Symptom**: A panic inside any thread holding a runtime diagnostics or correlation mutex permanently poisoned the lock, causing cascading `.lock().unwrap()` panics across the daemon.
- **Exact Root Cause**: `BrainRuntime` and `brain-observability` used `std::sync::Mutex` with `.lock().unwrap()`.
- **Code Invariant Enforced**: Replaced `std::sync::Mutex` with `parking_lot::Mutex` across `BrainRuntime` (`diagnostics`, `epoch`) and `brain-observability` (`correlation_index`), eliminating poison panics.
- **Regression Test**: `crates/brain-services/tests/adversarial_tier5_runtime.rs`.

### 27. Cross-Thread Deadlock in `HostContext::execute_tool`
- **Symptom**: Concurrent tool executions calling `HostContext::execute_tool` could deadlock or panic when bridging synchronous trait methods to async tool futures.
- **Exact Root Cause**: `HostContext::execute_tool` in `crates/brain-services/src/runtime/mod.rs` held a `service_locator.read()` lock guard across `tokio::task::block_in_place(|| handle.block_on(...))` and did not branch on `RuntimeFlavor::CurrentThread`.
- **Code Invariant Enforced**: Cloned the target `Arc` and explicitly dropped the `service_locator.read()` guard before blocking; branched on `handle.runtime_flavor()` to use `tokio::task::block_in_place` on multi-thread runtimes and `std::thread::scope` on current-thread runtimes.
- **Regression Test**: `crates/brain-services/tests/adversarial_tier5_runtime.rs`.

### 28. Silent Fallback Vectors (`[0.0; 384]`) & GIL Blocking in Python Providers
- **Symptom**: Network, timeout, or HTTP 401/500 errors during embedding or LLM calls returned silent `[0.0; 384]` dummy vectors or raw error strings while holding the Python GIL, polluting vector search rankings and blocking concurrent daemon workers.
- **Exact Root Cause**: Legacy providers in `daemon/brain/providers/` used synchronous Python `urllib.request` over PyO3 and caught exceptions by returning zeroed vectors.
- **Code Invariant Enforced**: Migrated `HeuristicExtractor`, `LlmMemoryExtractor`, `OllamaLlmProvider`, `OpenAiLlmProvider`, `AnthropicLlmProvider`, `OllamaEmbeddingProvider`, `OpenAiEmbeddingProvider`, and `OpenAiCompatibleEmbeddingProvider` to native Rust (`crates/brain-services/src/extraction/` and `crates/brain-services/src/providers/`) returning typed `Result<_, BrainError>` with zero dummy vector fallbacks, and wired them into `ServicesReadyPhase`.
- **Regression Test**: `crates/brain-services/tests/provider_tests.rs` (offline provider tests) and `crates/brain-services/tests/heuristics_tests.rs` (heuristic extraction and property tests).

### 29. Dangling Graph Edges After Heuristic Entity Cap Truncation
- **Symptom**: When `HeuristicExtractor` truncated extracted entities to the maximum node cap, edges referencing pruned entity IDs remained in the returned edge list, violating graph referential integrity.
- **Exact Root Cause**: Edge extraction ran before checking whether both `source` and `target` node IDs survived node-cap truncation.
- **Code Invariant Enforced**: Added a referential-integrity pruning pass at the end of `HeuristicExtractor::extract_raw` (`crates/brain-services/src/extraction/heuristics.rs`) that retains only edges whose `source` and `target` exist in the final node set and prevents self-loops (`source != target`).
- **Regression Test**: `crates/brain-services/tests/heuristics_tests.rs` and `crates/brain-services/tests/adversarial_tier5_extraction_provider.rs`.

### 30. Missing `LC_RPATH` When Testing `brain-services` with `--features python-plugins`
- **Symptom**: Running `cargo test -p brain-services --features python-plugins` on macOS failed at process launch with `dyld: Library not loaded: @rpath/Python3.framework/... (Reason: no LC_RPATH's found)`.
- **Exact Root Cause**: Cargo does not propagate `cargo:rustc-link-arg=-Wl,-rpath,...` from a dependency's build script (`crates/brain-python/build.rs`) to downstream test binaries in `crates/brain-services`.
- **Code Invariant Enforced**: Added `crates/brain-services/build.rs` gated strictly on `CARGO_FEATURE_PYTHON_PLUGINS` to emit macOS Python framework `@rpath` linker flags when `python-plugins` is enabled.
- **Regression Test**: `cargo test -p brain-services --features python-plugins --test runtime_tests` and `otool -L target/debug/brain-daemon` when inspecting the packaged daemon linkage.

### 31. Environment Variable Leakage Across `services_ready_phase_tests`
- **Symptom**: If `test_services_ready_phase_llm_extractor_env` panicked before finishing (such as when `MockServer::start().await` failed to bind `127.0.0.1:0` in a restricted sandbox), `BRAIN_MEMORY_EXTRACTOR="llm"` leaked into subsequent tests and broke `test_services_ready_phase_defaults_fallback`.
- **Exact Root Cause**: `std::env::set_var("BRAIN_MEMORY_EXTRACTOR", "llm")` was called without an RAII drop guard, and `MockServer::start().await` panicked on `PermissionDenied` before reaching the test's in-process mock fallback.
- **Code Invariant Enforced**: Added an RAII `EnvVarGuard` (`Drop` removes the env var even on unwind) and a non-panicking `try_start_mock_server()` helper in `crates/brain-services/tests/services_ready_phase_tests.rs`.
- **Regression Test**: `cargo test -p brain-services --test services_ready_phase_tests`.

### 32. `DaemonHost` Shutdown `Arc` Unwrap Leak & Unwired `BrainApplication` Compiler/Providers
- **Symptom**: On daemon exit, `Arc::try_unwrap(brain_runtime)` in `DaemonHost::run_server` always failed and skipped `brain_runtime.shutdown()`, while `BrainApplication` only registered `DeterministicMockProvider` in `ModelGateway` and ran `compile_knowledge` against a throwaway empty `KnowledgeIR::new()`.
- **Exact Root Cause**: `DaemonHost::run_server` kept `app_runtime` and the spawned `start_health_server` task alive across `Arc::try_unwrap(brain_runtime)`, and `BrainApplication` created new ephemeral `KnowledgeCompiler::new()` / `KnowledgeIR::new()` instances per call instead of hydrating from `SqliteStorage` and sharing compiler state.
- **Code Invariant Enforced**: Aborted `health_handle`, shut down and dropped `app_runtime` before `Arc::try_unwrap(brain_runtime)` (with a non-consuming `BrainRuntime::shutdown_ref(&self)` fallback), bridged native `ChatAgent` providers into `ModelGateway` via `ChatAgentModelProvider`, connected `list_reflection_proposals` to live `ReflectionEngine` findings, and hydrated `KnowledgeIR` from `SqliteStorage` into a shared `Arc<KnowledgeCompiler>`.
- **Regression Test**: `cargo test -p brain-application --test application_tests`.

### 33. Fractional Recall@K Masking Multi-Fact Personalization Failures (`PersonalizationV4`)
- **Symptom**: Standard fractional `compute_recall_at_k` reported `0.50` recall on 2-fact personalization queries where only 1 of 2 required implicit constraints was retrieved, masking complete downstream answer failures on zero-lexical-overlap scenarios.
- **Exact Root Cause**: `crates/brain-services/src/retrieval/eval_harness/metrics/recall.rs` only computed fractional hit rate ($|\text{retrieved}_K \cap \text{expected}| / |\text{expected}|$) and lacked a joint conjunction metric ($F_1 \land F_2 \land \dots \land F_m$) or an Always-On Constraint Subgraph decorator.
- **Code Invariant Enforced**: Added `compute_all_facts_recall_at_k`, `ConstraintAugmentedRetriever`, `Pv4HarnessAdapter`, and `compute_complementarity` in `crates/brain-services/src/retrieval/eval_harness/personalization_v4.rs` to evaluate joint All-Facts Recall@K and Oracle Hybrid Union rates alongside standard `run_benchmark` metrics.
- **Regression Test**: `cargo test -p brain-services retrieval::eval_harness::personalization_v4::tests`.

### 34. Native Build Failure in Personalization and Provider Wiring
- **Symptom**: The services and daemon targets failed to compile from the working source.
- **Exact Root Cause**: The personalization evaluator called `Uuid::new_v5` without enabling UUIDv5. The application used its dev-only config dependency, a private model settings field, and an incomplete provider match.
- **Code Invariant Enforced**: UUIDv5 is enabled for `brain-services`. The application declares its runtime config dependency, reads the provider through its accessor, and handles every configured provider variant.
- **Regression Test**: `cargo build --locked -p brain -p brain-daemon` and `cargo test --locked -p brain-services retrieval::eval_harness::personalization_v4::tests`.

### 35. Parallel Portable Runtime Fixtures Shared a Directory
- **Symptom**: The incomplete-runtime test intermittently found a complete Python runtime while running beside the other cases.
- **Exact Root Cause**: The temporary directory name combined the process ID and wall-clock nanoseconds, which could collide across test threads.
- **Code Invariant Enforced**: Each fixture uses a UUID directory and removes its own tree on drop.
- **Regression Test**: `cargo test --locked -p brain-daemon --test portable_runtime_tests` with parallel tests.

### 36. Personalization Benchmark Mixed Constraints from Different Collections
- **Symptom**: Other collections occupied slots in a scenario’s top five, and one fixture shared incidental lexical terms with all of its expected facts.
- **Exact Root Cause**: The benchmark materialized several collections into one database and appended every collection’s constraints without rank priority. The infrastructure question repeated terms from its fact documents.
- **Code Invariant Enforced**: The benchmark binds each scenario to its own documents, ranks its constraints above query-dependent candidates, and preserves the zero-overlap condition in the synthetic fixture.
- **Regression Test**: `cargo test --locked -p brain-services --lib retrieval::eval_harness::personalization_v4::tests`.

### 37. Session Routing and Context Types Collided During Main Integration

- **Symptom**: The combined sources failed to compile due to duplicate session-routing declarations and a context service that no longer satisfied its thread-safety contract.
- **Root cause**: Both lines of work introduced the same runtime-path and strict session-ID helpers. The newer repository traits also permit transaction-borrowed views without thread-safety supertraits.
- **Invariant**: Each runtime and session-routing helper has one definition. The asynchronously shared context service requires `Send + Sync` on the repository objects it owns while the repository traits still support transaction-borrowed views. The packaged daemon initializes its selected Python runtime before starting Tokio.
- **Regression test**: `cargo check --locked --workspace --all-targets`, the context and frontend session contract suites, and portable runtime tests in `cargo xtask verify`.

### 38. Linux Lint Compiled macOS-Only Declarations

- **Symptom**: Linux CI rejected the integrated branch with unused Seatbelt declarations, an unnecessary return in the Python plugin build script, and a manual clamp in Landlock.
- **Root cause**: Some declarations and the plugin feature guard were compiled outside the platform blocks that used them. Landlock expressed the descriptor limit through chained bounds.
- **Invariant**: Seatbelt state and profile generation compile only where they are used. The Python framework search runs only for macOS plugin builds. Landlock keeps the established descriptor bounds with `clamp`.
- **Regression test**: Linux `cargo clippy --workspace --all-targets -- -D warnings` and `cargo xtask verify` in ordinary CI.
