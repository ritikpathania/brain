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
