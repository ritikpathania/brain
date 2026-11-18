# Regression Prevention and Invariants

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
