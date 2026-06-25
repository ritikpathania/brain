<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
|------|----------|
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.

## CLI TUI Design System

When writing or modifying Terminal User Interface (TUI) components in the `cli/` directory, always adhere to the design system specified in `DESIGN.md`:

1. **Use Theme Tokens Everywhere**:
   - Wrap all text in `<ThemedText>` and boxes in `<ThemedBox>` imported from `components/design-system`.
   - Never use raw RGB, hex, or plain ANSI colors directly in layout components. Use the union of defined `ColorToken` strings (e.g. `'claude'`, `'success'`, `'error'`, `'warning'`, `'inactive'`, `'text'`).

2. **Precomputed Theme Maps**:
   - The `<ThemeProvider>` precomputes active theme mappings to minimize runtime lookup overhead. Components resolve colors directly via `<ThemedText color="claude">` or the precomputed `theme` object.
   - Access theme properties via the custom `useTheme()` hook:
     ```typescript
     const { theme, themeType, setTheme } = useTheme();
     ```

3. **Standard Layout and Primitives**:
   - Favor higher-level headless components (`<Alert>`, `<Progress>`, `<Toast>`) and themed layout containers (`<Panel>`, `<Card>`) over raw `<ThemedText>` or `<ThemedBox>` usage wherever possible. This ensures semantic style encapsulation.
   - Use `<LogoV2>` for logo wordmarks.
   - Use `<Divider title="Section title" />` to separate layouts.
   - Use `<Spinner label="Rotating action text" />` for async loading states, which implements standard Braille-dot loops and odd/even frame shimmer breathing.
   - Use `<StatusLine>` as the pinned application footer.

4. **Theme Switch Precedence**:
   - The TUI client dynamically resolves the active theme with the following precedence: CLI argument (`--theme` / `-t`) → Environment Variable (`BRAIN_THEME`) → Terminal background auto-detection → Default (`'dark'`).

5. **Responsive Layouts**:
   - Design layouts to dynamically resize (`SIGWINCH`) using flexbox properties (`flexGrow`, `flexShrink`) and percentage widths instead of hardcoded column widths.
   - Handle compact terminal widths (< 80 columns) gracefully by simplifying output segments.

## CLI TUI Testing & Verification

When modifying or adding components to the TUI client:

1. **Verify Using Scenarios**: Use the modular scenarios (`ThemeScenario`, `AlertScenario`, `SpinnerScenario`, `ResizeScenario`, `ToastScenario`, `HistoryScenario`) defined in `verify_tui.tsx` to manually test and inspect components.
2. **Write Golden Snapshot Tests**: Add new correctness tests inside `verify_tui.test.tsx` using `ink-testing-library` and snapshot comparisons to cover interactions (like Arrow navigation or state toggles) in a deterministic, time-independent manner.
3. **Run Performance Profiling**:
   - Execute the deterministic React Profiler benchmark (`bun run benchmark:render`) to verify render commit performance.
   - Run the trend comparison tool (`bun run benchmark:compare --last 5`) to compare current results against preceding runs or a baseline without failing CI by default.
