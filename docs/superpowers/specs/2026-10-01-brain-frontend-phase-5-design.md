---
status: draft
owner: cli
canonical: false
review_cycle: static
last_reviewed: 2026-10-01
applies_to: v1.1+
---

# Brain Frontend Phase 5 — Standalone Product Distribution Design

**Stage:** The conversational design is approved. This written specification is
pending review; it is not implementation or release-verification evidence.

**Source plan:** [Brain frontend consolidation](../../audits/BRAIN_FRONTEND_CONSOLIDATION_PLAN.md),
Phase 5. The [architectural invariants](../../architecture/ARCHITECTURAL_INVARIANTS.md)
continue to govern protocol, state, and presentation ownership.

## 1. Outcome and scope

An extracted Brain distribution must launch its first-party terminal frontend
through `brain ui` from an unrelated working directory, without a repository
checkout, system Bun, a frontend-root override, or either private reference tree.
It must retain the verified, artifact-relative CPython daemon runtime.

Phase 5 changes production build, artifact validation, launcher resolution,
release assembly, blocking verification, and current install/release guidance.
It does not change UI behavior, UDS contracts, daemon execution semantics,
presentation hierarchy, or the Phase 1–4 dependency direction. It does not remove
the legacy shell or start Phase 6.

## 2. Current release boundary

The existing `package.sh` builds only the release daemon. The daemon packaging
script assembles `bin/brain-daemon`, `runtime/python`, its file-digest inventory,
and `runtime-manifest.json`. macOS linkage is relocated to
`@rpath/libpython3.12.dylib`; Linux uses artifact-relative loader resolution.

The release workflow builds daemon-only archives on native macOS arm64 and
Linux x86_64 runners. Publication is still conditional on the obsolete
`BRAIN_AUTHORIZED_FRONTEND_RELEASE` variable. Development `brain ui` instead
executes Bun against a structurally validated source package. These are two
different operational models today; Phase 5 joins them without changing the
daemon runtime layout.

## 3. Execution model decision

Use a **Bun standalone executable**, built from the owned frontend entrypoint
and frozen dependencies with **Bun 1.4.4**. The executable includes the Bun runtime
and bundled application dependencies; target hosts do not need Bun installed.
The frontend version remains the package version, currently `0.1.0`.

| Alternative | Trade-off | Decision |
| --- | --- | --- |
| Bundled JavaScript with external Bun | Smaller application payload, but requires a separately installed, versioned runtime and runtime discovery. | Not selected. |
| Bundled JavaScript plus a separate bundled Bun | Explicit runtime, but an additional executable and more runtime files to validate and deliver. | Not selected. |
| Standalone executable | Larger per-platform artifact and native signing obligations, but one fixed entrypoint and no target Bun discovery. | Selected. |

Build natively on each supported release runner rather than claiming untested
cross-compilation equivalence. Verify the actual executable's platform,
architecture, native library dependencies, and required assets. Bundled Bun does
not imply that operating-system libraries or platform requirements disappear.
macOS ad-hoc signing is not Developer ID signing or notarization; do not claim
either without separate evidence.

## 4. Build and identity contract

Add `bun run build` to `packages/brain-frontend`. The release path must first run
`bun install --frozen-lockfile`, full typecheck, unit tests, and configured
real-daemon E2E with zero required skips. Build only owned production source;
do not ship frontend source, `node_modules`, test fixtures, or source maps.

The compile operation must disable automatic dotenv, bunfig, tsconfig, and
package-manifest loading by the resulting executable. An unrelated working
directory or a user's home configuration must not select another entrypoint,
preload, or runtime configuration. Validate the pinned Bun version before build;
never install or discover a replacement runtime automatically.

Provide a noninteractive build-identity diagnostic that does not require a
socket or initialize the terminal UI. It reports compiled frontend identity,
version, source SHA, target, and runtime model. Release identity is compiled
into the standalone artifact; caller-supplied identity overrides must not
masquerade as a different packaged frontend. Development configuration behavior
remains intact.

The source identity is a full 40-character Git SHA. Release CI builds from its
checked-out release commit, and validates the release tag against Brain's Cargo
product version. A local dirty-tree artifact must be labelled as such and carry
a deterministic source-input fingerprint; it is local verification evidence,
not a clean tagged-release reproduction. Record Bun version and lockfile digest
so reproducibility does not depend on the commit label alone.

## 5. Distribution layout and manifests

```text
brain-dist/
  bin/
    brain
    brain-daemon
  frontend/
    brain-frontend/
      brain-frontend
      manifest.json
  runtime/
    python/
    python-files.json
  runtime-manifest.json
  product-manifest.json
  licenses/
```

The frontend manifest uses a versioned schema and records:

- Identity `brain-frontend` and package `@brain/frontend`.
- Brain product version and independently versioned frontend package version.
- Full source SHA, source-tree status/fingerprint, and target triple.
- Runtime model `bun-standalone`, pinned Bun version, and lockfile digest.
- Fixed relative entrypoint `brain-frontend` and its SHA-256.

The product manifest records product version, source identity, target, component
manifest locations, CLI digest, and shipped contents. The existing daemon
runtime manifest and Python inventory remain authoritative for that component.
Brain CLI and daemon must have the same product version. Frontend package
version need not equal that version, but its manifest and compiled identity must
agree. Existing protocol negotiation remains mandatory.

Deliver the actual licenses/notices required by the bundled runtime and
dependencies, including the existing CPython obligations. Missing notice
evidence is a packaging failure, not permission to assert rights from package
metadata. No private/recovered implementation, prompts, assets, or artifacts
may be included. Artifact digests detect mismatches; they do not establish
publisher authenticity or replace a signature/security review.

## 6. Launcher resolution and validation

Resolution order:

1. Explicit `BRAIN_FRONTEND_ROOT` development override.
2. Packaged frontend relative to the canonicalized Brain executable.
3. In-repository development package relative to the executable's workspace.

An empty or invalid explicit override fails without fallback. A detected but
incomplete, malformed, incompatible, or tampered packaged installation also
fails without falling back to source. Repository development resolution is
allowed only when no packaged installation is present.

Packaged detection uses the distribution's product/component manifests, not
an arbitrary adjacent `package.json`. Validate schema, identity, target,
versions, fixed entrypoint, executable permission, path containment, and digest
before running it. Reject path traversal and symlinks resolving outside the
validated artifact root. Compare its noninteractive compiled identity against
the manifest. Never search CWD, home directories, private checkouts, or the
legacy shell. Manifest validation is bounded and produces actionable errors.

Execute the packaged frontend directly with argument-based process invocation;
do not construct a shell command. Preserve the effective Brain socket, unchanged
daemon preflight, terminal ownership, and child exit/signal semantics. Existing
sibling-daemon discovery resolves `bin/brain-daemon`. The development branch
still resolves and validates Bun, then runs the existing package start command.

Packaged launch must remove runtime entrypoint/preload overrides, including
`BUN_BE_BUN`, `BUN_OPTIONS`, and `NODE_OPTIONS`. It must not inject Claude config
or private paths. `BRAIN_FRONTEND_RUNTIME` continues to affect only the explicit
development runtime, never the bundled executable. Preserve required Brain
runtime/configuration inputs and caller terminal capabilities.

## 7. Existing daemon runtime integration

Compose the verified daemon runtime layout with the CLI and frontend; do not
redesign CPython acquisition, initialization, library relocation, or service
behavior. The frozen CPython pin is currently **3.12.13**.

One necessary verifier correction is identified: the clean-extraction compiled
extension probe currently constructs a macOS `.dylib` path even in the Linux
verification branch. Resolve the library from the existing daemon manifest's
`python.library` field instead, with containment validation and regression
coverage. Preserve the proof that a real `lib-dynload` module imports from
inside the extracted runtime, not the build interpreter.

## 8. Blocking verification and clean extraction

Replace the external-source publication guard with required first-party build
and artifact gates; do not merely remove the guard. Native platform release
jobs must build, assemble, archive, extract, and test the complete product before
publication. No skipped frontend gate counts as release proof.

The product smoke must:

1. Extract the actual archive into a fresh temporary root; preserve permissions.
2. Use isolated HOME/XDG, config, socket, database, PID, and log paths, and an
   unrelated CWD. Remove development override variables and Bun from PATH.
3. Launch extracted `bin/brain ui`, exercising sibling daemon startup/preflight
   and the validated standalone frontend, not a source-tree runner.
4. Use a real PTY and the existing deterministic mock provider to create a
   session, submit one prompt, observe authoritative completion, and load that
   session/history. Synchronize on bounded observable transitions, not broad
   sleeps. Do not introduce production mock behavior for this test.
5. Assert handshake identity, no duplicate submission, clean frontend exit,
   and cleanup of the daemon and temporary state on success or failure.
6. Reject build-host paths in executable/runtime files: `/Users/`, `/home/`,
   checkout/private/reference paths, and build-cache roots. Inspect native
   loader paths and ensure all non-system dependencies are explicit.

Also retain the independent daemon runtime smoke and compiled-extension import
proof. Repeat the macOS product extraction smoke at least three times. Test
invalid manifest/digest/identity/entrypoint, invalid explicit override, conflicting
adjacent/CWD packages, ambient runtime overrides, and development launch.

The same native Linux job must execute the complete smoke. Until its completed
result exists, Linux packaging is **configured/pending**, not verified. Local
macOS evidence cannot stand in for a Linux execution result.

Repository gates remain: locked all-target/all-feature Rust check, locked
all-feature workspace tests, Rust formatting, active docs validation, frontend
frozen install/typecheck/unit/E2E/build, workflow syntax, and scoped diff checks.
Report actual completed exits and any fresh failure honestly.

## 9. Changes permitted and completion boundary

Expected areas are frontend build/bootstrap metadata and focused tests; launcher
resolution and launcher tests; product assembly/verification scripts; the narrow
daemon verifier correction; release workflow; and current install/release docs.
Presentation source and daemon protocol remain unchanged. Preserve all unrelated
dirty-tree work and do not commit it as part of this increment.

Append Phase 5 evidence to the consolidation plan only after verification, and
reassess retirement gates without deleting anything. Historical audit/provenance
records remain historical. Independent first-party artifacts replace the old
external-source release requirement; neither private reference tree becomes an
authorized release input.

Phase 5 is complete only with a runnable, validated macOS extracted product,
blocking native platform release checks, explicit Linux evidence/pending status,
green required regression gates, and no legacy deletion. Signing/notarization,
native selection/clipboard, and Phase 6 caller inventory remain separate work.
