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

When writing or modifying Terminal User Interface (TUI) components in the `crates/brain-tui/` directory, always adhere to the design system specified in `crates/brain-tui/src/ui/theme.rs`:

1. **Use Theme Tokens Everywhere**:
   - Never use raw RGB, hex, or plain ANSI colors directly in layout components. Use semantic colors defined in `Theme`.

2. **Precomputed Theme Maps**:
   - Access theme properties via the active TUI client theme structures to minimize runtime lookup overhead.

3. **Standard Layout and Primitives**:
   - Favor themed layout containers over raw text or box primitives. This ensures semantic style encapsulation.
   - Design layouts to dynamically resize (`SIGWINCH`) using flexbox properties (`flex-grow`, `flex-shrink`) and percentage widths instead of hardcoded column widths.
   - Handle compact terminal widths (< 80 columns) gracefully by simplifying or hiding sidebar panels.

## CLI TUI Testing & Verification

When modifying or adding components to the TUI client:

1. **Verify Using Integration Tests**: Run `cargo test -p brain-tui` to check state reducer invariants and layout computations.
2. **Run Performance Profiling**: Compile the workspace in release mode using cargo and measure peak RSS memory usage and frame draw latency to check for regressions.

## UDS Streaming Protocol & Typewriter Renderer

When working with queries or communication between the Rust Daemon and the TUI Client:

1. **Monotonic Tagged Stream Events**:
   - Communication uses `StreamEvent` tagged enum variants (`stream_start`, `stream_progress`, `stream_chunk`, `stream_end`, `stream_cancelled`).
   - Sequence numbers must increment monotonically within a stream.

2. **Two-Stage Client Queue Pipeline**:
   - The TUI buffers incoming network chunks into a typewriter queue, draining them sequentially to create a smooth rendering effect.
   - Handles network transport completion independently from the typewriter rendering queue drain.

## Technical Walkthrough & Documentation Invariant

When claiming work is complete, performing walkthroughs, or explaining architectural changes:
- Update the local walkthrough artifact (in the system-generated `<appDataDir>/brain/<conversation-id>/walkthrough.md` artifacts directory) to match the work done.
- Always update the unified, project-level documentation at [WALKTHROUGH.md](file:///Users/ritikpathania/Developer/PyCharm/brain/WALKTHROUGH.md) directly.

