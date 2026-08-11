# Claude Code vs. Brain TUI — Feature Comparative Matrix
> Research pass · Brain TUI Audit · 2026-08-10
> Legend: **Full** (completely implemented), **Partial** (implemented with gaps), **Stub** (visual/type placeholder), **Missing** (not present)

---

## 1. Core TUI Engine & Architecture

| Feature | Claude Code | Brain TUI | Diff Category | Notes |
|---|---|---|---|---|
| Tech Stack | React + Ink (TS) | Ratatui (Rust) | Architectural | Ink uses JS event loop; Brain uses Rust Tokio channels |
| Layout Engine | Yoga Flexbox | Ratatui Layout (Rect split) | Parity | Both use flexbox/constraint-based row-column math |
| Component Model | React Components | ViewModels + Pure Widgets | Parity | Brain decouples state rendering into stateless draw functions |
| Unidirectional Flow | React State Hooks | DDD Event Envelope + Reducers | Parity | Brain uses strict DDD invariants; Claude uses React context |
| Screen Buffer | `<AlternateScreen>` | `crossterm::terminal::EnterAlternateScreen` | Full Parity | Both run in dedicated terminal buffer |
| Input Event Multiplexing | Node process input | `tokio::select!` crossterm events | Full Parity | Both asynchronously listen to raw terminal input |

---

## 2. Startup & Home Surface

| Feature | Claude Code | Brain TUI | Diff Category | Notes |
|---|---|---|---|---|
| ASCII Mascot / Branding | Full `<Clawd>` logo | Full `MascotWidget` | Full Parity | Both feature prominent ASCII art headers |
| Condensed Startup Mode | `<CondensedLogo>` for returning users | None | Brain Missing | Claude collapses header if onboarding complete |
| Proportional Prompt Anchor | Bottom-pinned below history | ~67% height bounded clamp | Visual Diff | Brain anchors Home container proportionally; Claude anchors to bottom |
| Right-side Dashboard Widgets | FeedColumn (activity, changelog) | SystemStatus + MemoryContext | Functional Diff | Claude shows release notes; Brain shows daemon & memory stats |
| Dynamic Layout Mode | `horizontal` (≥70) vs `compact` | Dynamic sidebar clamp | Full Parity | Both collapse panels on narrow terminals |

---

## 3. Input & Command Prompt

| Feature | Claude Code | Brain TUI | Diff Category | Notes |
|---|---|---|---|---|
| Multiline Text Input | Supported (Shift+Enter) | Supported (`Paragraph` widget) | Full Parity | Both handle multiline editing |
| Border State Color Coding | 4 states (gray/orange/purple/red) | Active theme border tokens | Parity | Claude uses state-specific border highlights |
| Vim Input Mode | Supported (`VimTextInput`) | Not implemented | Brain Missing | Claude has full Vim bindings support |
| Input History Navigation | Up/Down arrow history | **Stub** (Model exists, events unrouted) | Brain Gap | Brain's `HistoryStore` is disconnected from `event.rs` |
| OS Clipboard Pasting | Alt+V / Ctrl+V image & text paste | **Stub** (`Event::Paste` unhandled) | Brain Gap | Brain drops paste events in `event.rs` |
| Slash Completion Popup | Typeahead fuzzy menu | Floating overlay completion | Full Parity | Both provide floating slash-command popups |
| Mode Badges (Plan/Auto) | Footer mode indicator badge | Status Footer mode text | Parity | Claude renders inline badge; Brain shows in status |

---

## 4. Command Ecosystem & Search

| Feature | Claude Code | Brain TUI | Diff Category | Notes |
|---|---|---|---|---|
| Registered Slash Commands | 60+ commands | 15+ commands | Expansion Area | Claude has broader CLI utility scope |
| Global Workspace Search | Ripgrep stream (Ctrl+Shift+F) | Palette search (Ctrl+K) | Functional Diff | Claude streams file matches; Brain uses node graph search |
| Quick File Open | Fuzzy file search (Ctrl+Shift+P) | Integrated Palette | Functional Diff | Claude separates file open from command palette |
| Interactive Command Palette | FuzzyPicker dialog | Floating `PaletteWidget` | Partial Parity | Brain's parameter/confirm stages are visual stubs |
| Live Theme Switching | `/theme` with live preview | `/theme` or `Ctrl+T` palette switch | Partial Parity | Claude previews themes on hover before confirming |

---

## 5. Layout & Workspace Rendering

| Feature | Claude Code | Brain TUI | Diff Category | Notes |
|---|---|---|---|---|
| Virtualized Scroll History | `VirtualMessageList` | ViewportIndex line virtualizer | Full Parity | Both maintain O(1) rendering costs for long chats |
| Unseen Message Divider | "N new messages" pill when scrolled up | Unseen line divider indicator | Full Parity | Both track unread content while user is scrolled up |
| Multi-pane Workspace | Single chat stream focus | Sidebar + Chat + Graph Inspector | Brain Native | Brain supports 3-pane exploration mode |
| Markdown Formatting | Bold/Italic/Code/Tables | `ratatui-markdown` styling | Full Parity | Both render formatted Markdown in terminal |
| Dynamic Syntax Highlighting | `HighlightedCode` | Syntect / ANSI highlighting | Full Parity | Both highlight code blocks |

---

## 6. Footer & Status Reporting

| Feature | Claude Code | Brain TUI | Diff Category | Notes |
|---|---|---|---|---|
| Status Bar Location | Fixed single-row footer | Pinned `StatusFooter` widget | Full Parity | Bottom-anchored status indicators |
| Status Bar Customization | User hook command script | State-driven ViewModel | Functional Diff | Claude allows shell script status bar override |
| Loading / Streaming Spinner | Braille-dot spinner + verb cycle | Animated braille spinner | Full Parity | Both use Braille dot frames `⠋⠙⠹...` |
| Shimmer Animation Framework | Base ↔ Shimmer token pairs (80ms) | Palette token resolution | Claude Native | Claude alternates lighter RGB tokens for breathing effect |
| Subagent Identity Colors | 8 dedicated color tokens | N/A (Single-agent session) | Claude Native | Claude assigns colors to sub-agents |

---

## 7. Themes & Visual Tokens

| Feature | Claude Code | Brain TUI | Diff Category | Notes |
|---|---|---|---|---|
| Truecolor (24-bit RGB) | Supported | Supported (`ThemeToken` maps) | Full Parity | High-fidelity RGB color depth |
| Light / Dark Auto-Detection | OSC 11 + `$COLORFGBG` query | `MacOSPollingProvider` | Full Parity | Automatic system dark/light sync |
| Colorblind / Daltonized Themes| `dark-daltonized` & `light-daltonized` | High Contrast palette mode | Parity | Accessibility color modes |
| ANSI-16 Fallback | `dark-ansi` & `light-ansi` | Fallback terminal colors | Full Parity | Degrades gracefully on older TTYs |
