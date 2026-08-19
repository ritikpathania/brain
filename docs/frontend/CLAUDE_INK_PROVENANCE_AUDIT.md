# Claude / Ink Frontend Provenance & Architecture Audit

**Target Package**: `packages/brain-frontend`  
**Reference Specification**: Claude Code v2.1.232 (`/Users/ritikpathania/Developer/src`)  
**Backend System**: Rust Brain Daemon (`apps/brain`, `crates/brain-*`)  
**Audit Date**: 2026-08-15  

---

## 1. Architectural Boundary & Responsibilities

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                          packages/brain-frontend                            │
│                                                                             │
│  TypeScript / React / Ink                                                   │
│  ├── Terminal Theme Detection (OSC 11 / $COLORFGBG luminance probe)         │
│  ├── Root ThemeProvider & ThemedText / ThemedBox design system primitives   │
│  ├── Native terminal foreground semantics                                   │
│  ├── Ink / Yoga layout engine (calculateLayoutDimensions)                   │
│  ├── Atomic rounded border primitives                                       │
│  ├── Typography & Unicode stringWidth measurement                          │
│  ├── Welcome screen branding (LogoV2 / Clawd / FeedColumn)                  │
│  ├── Prompt composer (open-border, bold ❯ pointer)                          │
│  ├── Help menu (3-column fixed widths: 24 / 35 / unconstrained)             │
│  ├── Footer & status line (❚❚ manual mode, shortcuts, agents hints)         │
│  ├── Keyboard & interactive useInput event handling                         │
│  └── Raw ANSI SGR truecolor stream emission                                 │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       │ UDS JSONL Wire Protocol
                                       │ (/tmp/brain.sock)
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                             Rust Brain Daemon                               │
│                                                                             │
│  Headless Rust Backend                                                      │
│  ├── Brain Engine & Facade Orchestration (brain-services)                   │
│  ├── Knowledge Graph & Relational Memory (brain-domain)                     │
│  ├── SQLite Storage, WAL & Event Logs (brain-storage)                       │
│  ├── Session State & Domain History (brain-session)                         │
│  └── Sandboxed Tool Execution (brain-tools)                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Component Mapping & Provenance Table

| Subsystem / Concern | Claude Reference Source (`Developer/src`) | Brain TypeScript Implementation (`packages/brain-frontend/src`) | Architectural Status |
| :--- | :--- | :--- | :--- |
| **Root Render & Theme Wrap** | `src/ink.ts` (`withTheme`) | `src/main.tsx` & `src/App.tsx` | Literal Port wrapped in `<ThemeProvider>` |
| **Terminal Luminance Detection** | `src/utils/systemTheme.ts` | `src/utils/systemTheme.ts` | Dynamic `$COLORFGBG` + OSC 11 probe |
| **Theme Provider & Context** | `src/components/design-system/ThemeProvider.tsx` | `src/components/design-system/ThemeProvider.tsx` | Literal Context Provider with `useTheme` |
| **Native FG & Text Theming** | `src/components/design-system/ThemedText.tsx` | `src/components/design-system/ThemedText.tsx` | Native FG preserved (uncolored text -> undefined) |
| **Box & Border Color Theming** | `src/components/design-system/ThemedBox.tsx` | `src/components/design-system/ThemedBox.tsx` | Semantic color resolution to raw hex |
| **Unicode String Measurement** | `src/ink/stringWidth.ts` | `src/utils/stringWidth.ts` | Grapheme segmentation + EAW + Emoji |
| **Responsive Sizing Math** | `src/utils/logoV2Utils.ts` | `src/components/LogoV2/logoV2Utils.ts` | `calculateLayoutDimensions()` & `optimalLeftWidth` |
| **Welcome Box Branding** | `src/components/LogoV2/LogoV2.tsx` | `src/components/LogoV2/LogoV2.tsx` | Atomic top border, single-line metadata |
| **Mascot Art & Feeds** | `src/components/LogoV2/Clawd.tsx` | `src/components/LogoV2/Clawd.tsx`, `FeedColumn.tsx` | Terra-cotta Clawd `#D77757` & Feeds |
| **Prompt Composer** | `src/components/PromptInput/PromptInput.tsx` | `src/components/PromptInput/PromptInput.tsx` | Open-border round container, bold `❯` |
| **Help Menu Columns** | `src/components/PromptInput/PromptInputHelpMenu.tsx` | `src/components/PromptInput/PromptInputHelpMenu.tsx` | Columns: 24 / 35 / unconstrained |
| **Status / Footer Line** | `src/components/PromptInput/PromptInputFooter.tsx` | `src/components/PromptInput/PromptInputFooter.tsx` | `❚❚ manual mode on · ? for shortcuts` |
| **Workspace / Agents Screen**| `src/commands/agents/agents.tsx` | `src/components/WorkspaceView.tsx` | Literal Agents table & status layout |
| **Modal Command Palette** | `src/components/GlobalSearchDialog.tsx` | `src/components/GlobalSearchDialog.tsx` | Themed modal popup dialog |
| **ANSI Stream Verification** | Raw Terminal PTY Capture | `src/ansiCellGrid.ts` | 2D `TerminalCell` grid truecolor parser |

---

## 3. Pipeline Audits & Divergence Resolutions

### A. Theme Pipeline Audit
* **Claude Mechanism**: Claude determines terminal appearance via `$COLORFGBG` or OSC 11. `ThemedText` leaves uncolored text as `color={undefined}` so Ink utilizes the terminal's native foreground color (bold black in light terminal, bold white in dark terminal).
* **Previous Divergence**: Brain hardcoded `useState('dark')` and components passed `color={theme.text}` (`#FFFFFF`), rendering white text on light backgrounds.
* **Resolution**: Root `ThemeProvider` dynamically detects terminal luminance via `resolveThemeSetting('auto')` and propagates native foreground for unstyled text.

### B. Layout & Sizing Pipeline Audit
* **Claude Mechanism**: `calculateLayoutDimensions(cols)` allocates `leftWidth = optimalLeftWidth` (capped at 50) and `rightWidth = Math.max(30, availableForRight)`. Sizing strictly satisfies $totalWidth = leftWidth + divider (1) + rightWidth + padding (2)$.
* **Previous Divergence**: Brain had conflicting `gap={1}` and `paddingX={1}`, creating a 68-col inner requirement inside a 66-col container at 70×37, wrapping `g │` onto the next row.
* **Resolution**: Replaced manual gap math with strict $45 + 1 + 18 = 64$ inside `paddingX={1}` ($64 + 2 = 66$), producing exact 0-wrapping 70×37 layout.

### C. Metadata Pipeline Audit
* **Claude Mechanism**: `${modelDisplayName} · ${billingType}` on a single line, where `modelDisplayName = truncate(model, leftWidth - 20)`. CWD formatted via `getDisplayPath(cwd)` (`~/Developer/PyCharm/brain`).
* **Previous Divergence**: Brain split model and billing onto two lines, displacing the mascot and metadata distribution.
* **Resolution**: Reconstructed Claude's single-line metadata representation and middle-path truncation.

### D. ANSI & Verification Pipeline Audit
* **Claude Mechanism**: Emits 24-bit SGR Truecolor sequences (`\x1b[38;2;R;G;Bm`).
* **Previous Divergence**: Test oracle called `stripAnsi()`, discarding all color information and creating false passes.
* **Resolution**: Replaced `stripAnsi()` with `parseAnsiToGrid()`, asserting truecolor hex values, bold/dim attributes, and exact cell coordinates across all 5 standard viewports in both Dark and Light modes.
