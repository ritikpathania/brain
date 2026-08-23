# Inc 0 — Shim & Test Triage Notes

Companion to `2026-08-23-brain-shell-inc0-contracts-decoupling.md` Task 10.
Records every triage decision and its justification, per plan Step 2.

## Method correction

The plan's triage script matched `PROVIDED` module ids against imports that
carry a `.js` suffix (`types/message` vs `vendor/claude/types/message.js`),
flagging suites whose every import has a contracts twin. The script was
corrected to normalize the suffix before matching; the corrected doomed set
is what drove deletion.

## Deleted now — CC product surfaces / vendor harnesses

Shims: `bundledArtifactDiagramming`, `bundledVerify`, `claudeApiShim`,
`claudeForChromeMcp`, `feedConfigs`, `Clawd` (replaced by BrainMark),
`LogoV2`, `Notifications`, `PermissionRuleList`, `PromptInputFooter`,
`PromptInputFooterLeftSide`, `PromptInputFooterSuggestions`, `select`,
`Tabs`, `shellPermissionHelpers`, `permissionOptions`, `simplifySkill`,
`runSkillGeneratorSkill`, `fewerPermissionPrompts`, `codeReviewSkill`,
`doctorSkill`, `Opus1mMergeNotice`. None had a Brain-keeper role; each
either wrapped a vendor original unchanged or implemented a CC product
surface that dies with its feature. Nothing stubbed in their place:
every preload id they used to serve now resolves to the vendor original
(see "Redirect integrity" below).

Exception: `brainQuery` / `brainQueryDeps` were initially deleted, which
broke ~10 deferred suites that import the query seam through them; both
were restored. `brainQuery.ts` injects `productionDeps()` into the
vendor query generator; `brainQueryDeps.ts` builds it from Brain seams
(`createBrainCallModel` over shared client/store/provider/emitter) plus
the vendor harness fields the QueryDeps contract requires —
`microcompact: microcompactMessages`,
`autocompact: autoCompactIfNeeded`, `uuid: randomUUID` (vendor
`query/deps.ts` rejects a deps object supplying only `callModel`). The
Inc 1 native query loop replaces both files.

Tests: `test_local_tools_capability_suite`, `commandExecutionMatrix`,
`debugMain`, `negativeDependencyVerification` (its zero-analytics /
zero-OAuth-diff gates are superseded by Task 12's no-vendor grep gates),
`test_mcp_surface_suite`, `test_permission_rule_ux_suite`,
`themeIntegration` (vendor /theme + /vim surfaces), `inkRenderTest`
(also the parse-broken file that aborted whole-tree tsc at HEAD),
`importTest`, `frontend-contract/parityClosureContracts` (asserts parity
against vendor Feed/onboarding screens that die with vendor),
`toolExecutionMatrix`, `test_quarantine_boundary_suite`.
Files testing removed CC features died with their features; nothing stubbed.

## Swapped now — leaf consumers with contracts twins

- `src/test/frontend-contract/memorySeamIntegration.test.ts`:
  `vendor/claude/utils/messages.js` → `contracts/messages.js`; positional
  factory call updated to the object-param API.
- `src/shims/UserCommandMessage.tsx`: vendored ink → `compat/ink.js`,
  `utils/messages.js` → `contracts/messages.js`, SDK `TextBlockParam` →
  contract `TextBlock`, `COMMAND_MESSAGE_TAG` inlined as a Brain-owned
  literal.

Created `src/contracts/input.ts` (`PromptInputMode`, `VimMode`,
`VimInputState`; `ThinkingConfig` re-exported from `tools.js`). The fuller
vendor union (`orphaned-permission`, `task-notification`, queue priorities,
paste tracking) is re-derived in the composer increment, not copied here.

## Redirect integrity — the lesson the PTY suites taught

Deleting shims is not enough: every `preload.ts` rule that returned a
deleted shim's path became a live ENOENT. Bun-test suites never noticed
because none of them loads the shell startup graph — but the PTY runner
tests (every "Dimension 4 (Layer 1): multi-viewport" case across
overlay/composer/runtime/lifecycle contracts and multiViewportParity)
spawn the real shell, which died on launch at
`bundledSkillsIndex.ts → ./fewerPermissionPrompts.js`. One crash, 33
failing tests whose only symptom was "expected output missing".

Fixes applied:

- Repointed to vendor originals (all verified present): `select`,
  `permissionOptions`, `shellPermissionHelpers`,
  `services/api/claude`, `feedConfigs`, `Clawd`, `Opus1mMergeNotice`,
  `Notifications`, `PromptInputFooter{,LeftSide,Suggestions}`,
  `PermissionRuleList` (`permissions/rules/`), design-system `Tabs`
  (`components/design-system/`), `LogoV2`, `bundled/verify`.
  Self-import guards kept so originals don't re-enter their own rule.
- Removed the chrome-mcp onResolve; the synthetic `build.module`
  already serves `@ant/claude-for-chrome-mcp`.
- Stripped the five deleted CC-surface skill registrations from
  `bundledSkillsIndex.ts`; it now re-exports vendor
  `skills/bundled/index.js` only.
- `commandSuggestions.ts` type import of `SuggestionItem` moved from the
  deleted shim to the vendor original (runtime-elided, but tsc-fatal).

Rule for Task 12: after any shim deletion, sweep `preload.ts` for rules
whose resolved target no longer exists, and run one PTY smoke — bun-test
green does not prove the shell boots.

## Task 12 — final deletions and gate results

Vendor deletion forced the die-or-port call on every suite still
reaching the vendored tree:

- The four frontend-contract PTY suites (composer/lifecycle/overlay/
  runtime), their python runners, `vendorManifest`, `performanceProfiling`
  and the deferred shim-bound tests died: they exercise the vendor-era
  runtime surface (e.g. asserting a `Claude Code v…` banner) that the
  Task 11 entrypoint swap intentionally retired. `multiViewportParity`
  joined them for the same reason even though its test file greps clean —
  its runner asserts vendor parity of the shell frame.
- `phase1_components` … `phase4_certification` + `theme_integration_brain`
  imported the **vendored ink fork** (`vendor/claude/ink.js`); they were
  the entire "stock-ink render cluster" (44 of the baseline's 50 fails).
  They die with the fork; Inc 1 rebuilds the Brain component matrices on
  `compat/ink.js` once it has a working bun-test renderer.
- The seven migration-audit python scripts (compareSourceTrees family,
  strictDifferentialAudit, copyShimsToSrc, …) were one-off archaeology on
  the vendor tree.
- All remaining shims (31 files) had zero surviving consumers once those
  suites went; `src/shims/` no longer exists. tsconfig lost its
  `paths`/vendor include.

Gates at close: `bun test` → 106 pass / 5 fail / 111 tests across 23
files, where all five residual fails are the pre-existing behavioral
singles documented above (visualCellParity ×2, sessionSemantic,
brainTurnTransformer, memoryIntegration). `bun build src/main.tsx` →
BUILD_OK with zero vendor references. PTY smoke → skeleton frame renders
and exits cleanly on SIGINT.

Two environment notes for future increments:
- ink's optional peer `react-devtools-core` must be installed
  (devDependency) or `bun build` fails resolving it; runtime never loads
  it under `NODE_ENV=production`.
- A PTY harness must set the window size (`TIOCSWINSZ`) before launch;
  at 0×0 `useTerminalSize()` returns zero columns and any width-based
  layout collapses to one character per line.

## Deferred with vendor — die or port at Task 12 / Inc 1

These stay on vendor imports until Task 12 removes vendor. Each is either
(a) an orphaned piece of the old REPL composition Task 11 replaces, or
(b) a live suite exercising Brain seams through the temporary vendor query
harness, which Inc 1 rebuilds Brain-natively. None is load-bearing for
production after Task 11.

- Shims: `REPL`, `useTextInput`, `useVimInput`, `ThemePicker`,
  `ThemeProvider`, `UserPromptMessage`, `HighlightedThinkingText`,
  `OffscreenFreeze`, `StatusNotices`, `colorDiff`, `ShellCommand`,
  `doctorCommand`, `permissionsCommand`, `memoryCommand`, `memory`,
  `agentsCommand`, `AgentsWorkspaceDashboard`, `ListItem`, `LogSelector`,
  `useTerminalViewport`, `ansiTokenize`, `wrapAnsi`, `logUpdate`,
  `commandSuggestions`, `resumeCommand`.
- Tests: `thinkingReasoningBlocks`, `toolExecutionRoundTrip`,
  `layer2AdapterContract`, `e2eBrainBackendGate`, `cancellationRaces`,
  `contractHarness`, `udsTransportAdapter`, `brainTextAdapter`,
  `productValidationE2E`, `lifecycleContracts`, `composerContracts`
  (typeahead/FileIndex have no contracts twin), `overlayContracts`
  (permission-dialog option builders are vendor-only),
  `runtimeContracts`, `theme_integration_brain`.
- `architectureFitness.test.ts` asserts `main.tsx` imports vendor main;
  Task 11 rewrites both the entrypoint and this gate together.
