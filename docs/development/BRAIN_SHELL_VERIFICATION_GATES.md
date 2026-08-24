# Brain Shell Verification Gates

Canonical verification commands and machine-specific requirements for
`packages/brain-shell` and the Rust daemon. Established across Increments 0–4
(2026-08-23 → 2026-08-24); keep this document current when a gate changes.

## Bundle gate (canonical)

There is **no** `build` script in `packages/brain-shell/package.json`. The
bundle gate is the direct bundler call:

```bash
cd packages/brain-shell && bun build src/main.tsx --outdir dist --target bun >/dev/null 2>&1 && echo BUILD_OK
```

Do not write `bun run build` in plans or CI — it fails with "Script not found".

## Rust test toolchain requirement (this Mac)

Every `cargo` invocation must carry the rpath link-arg or test binaries abort
before `main` with a dyld error about `@rpath/Python3.framework`:

```bash
RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test ...
```

Why: daemon test binaries link pyo3's
`@rpath/Python3.framework/Versions/3.9/Python3` transitively
(services → brain-python → pyo3) and ship no LC_RPATH. `DYLD_*` environment
variables are ignored for `@rpath` loads with an empty rpath list, and
`/Library/Frameworks` holds `Python.framework` without the `3`. The real
`Python3.framework` lives under the Command Line Tools directory above.

## PTY smoke harness rules

The Python PTY smokes (`scripts/ptySmokeInc*.py`) must:

1. Set a window size via `TIOCSWINSZ` before exec — ink computes layout from
   it; a 0×0 pty collapses every row to one character per line.
2. Write each keystroke as its own chunk and pump ≥0.3 s between distinct
   keys — ink parses one stdin chunk as ONE keypress.
3. Strip ANSI escapes before matching rendered text.

**Stale-render trap (Inc 4):** the screen buffer accumulates everything ever
rendered. A substring expect for UI that appears more than once matches the
EARLIER occurrence instantly, so follow-up keystrokes race async setup (the
deny key landed in the still-active composer instead of dialog #2). For any
repeated UI element, wait on an occurrence count instead:
`clean(buf).count(needle) >= N`.

**Fixture snapshots mutate on every run:** the committed
`src/test/fixtures/pty/<inc>/*.txt` files are snapshots of one successful run;
spinner glyph rotation and cursor timing make reruns byte-different, so a gate
run leaves those tracked files modified. Restore them afterwards
(`git checkout -- <fixture dir>`); making FIXTURE_DIR env-overridable is the
proper future fix.

## Suite baselines

Full-suite runs are compared against documented baselines, not blind green:

- Shell (`bun test`, packages/brain-shell): five known pre-existing failures —
  visualCellParity ×2, sessionSemanticIntegration,
  brainMemoryIntegration, brainTurnTransformer (MemoryContextTransformer API
  drift from the salvage baseline).
- Daemon: `uds_generation_tests` and `uds_permission_roundtrip_tests`
  expected fully green as of e2b5af7 (session-contract fix).

Vendor/provenance scans grep only ADDED lines of the shell diff:

```bash
git diff <base>..HEAD -- packages/brain-shell/src/ | grep '^+' | grep -icE 'claude|anthropic|vendor'   # expect 0
git diff <base>..HEAD -- packages/brain-shell/src/ ':!packages/brain-shell/src/test' | grep '^+' | grep -icE 'claude|anthropic|vendor'   # expect 0
```
