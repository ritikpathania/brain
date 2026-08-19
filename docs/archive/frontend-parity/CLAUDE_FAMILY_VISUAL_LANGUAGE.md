# Brain TUI — Claude-Family Visual Design Language Contract
> Design System Specification · 2026-08-10
> Architecture: Pure Rust / Ratatui + Crossterm · ThemeToken Abstraction Boundary

---

## 1. Core Philosophy

Brain TUI adopts a **Claude-family design language** while preserving **Brain's native identity**:

- **Restrained & Calm**: Minimal visual noise, no gratuitous heavy borders or multi-nested boxes.
- **Typography-First**: Visual hierarchy expressed through bold/dim/muted text weight and positioning rather than heavy container boxes.
- **Whitespace-Driven**: Generous, intentional padding and horizontal rule dividers (`─`).
- **Warm Accent Palette**: Warm coral/orange primary brand tone paired with soft muted gray secondary text.
- **Document-First Flow**: Conversations render as clean text streams rather than isolated card bubbles.
- **Responsive Geometry**: Layouts adapt fluidly across terminal dimensions (`80x24` up to `182x53`) without hardcoded bounds.

---

## 2. Color System & Theme Token Rules

All styling must be resolved through `ThemeToken` via the `ActiveTheme` trait. Direct `Color::` usage outside `src/ui/theme/` is strictly forbidden and guarded by automated tests.

| Token | Semantic Role | Dark Palette Reference | Light Palette Reference |
|---|---|---|---|
| `ThemeToken::Primary` | Brand accent & headers | Warm Coral `rgb(215, 119, 87)` | Soft Orange `rgb(192, 86, 33)` |
| `ThemeToken::TextPrimary` | Body text | Light Gray `rgb(240, 240, 240)` | Charcoal `rgb(32, 33, 36)` |
| `ThemeToken::TextSecondary` | Accessory labels | Muted Gray `rgb(180, 180, 180)` | Slate `rgb(102, 102, 102)` |
| `ThemeToken::TextMuted` | Dimmed metadata | Dark Gray `rgb(120, 120, 120)` | Muted Gray `rgb(130, 130, 130)` |
| `ThemeToken::BorderSubtle` | Horizontal dividers | Dark Neutral `rgb(80, 80, 80)` | Muted Gray `rgb(180, 180, 180)` |
| `ThemeToken::BorderFocused` | Focused input border | Warm Coral `rgb(215, 119, 87)` | Soft Orange `rgb(192, 86, 33)` |
| `ThemeToken::Selection` | Selection highlight | Slate Blue `rgb(38, 79, 120)` | Soft Blue `rgb(180, 213, 255)` |

---

## 3. Layout & Geometry Rules

### A. Home Screen Composition
- On tall terminals (>24 rows), the prompt container is anchored at **~67% of screen height** via a flexible filler space underneath the status line, ensuring natural visual focus.
- On narrow terminals (<70 columns), sidebars collapse automatically.
- No redundant connection cards or heavy "Try" launchers.

### B. Prompt Interaction Anchor
- Prompt uses a single clean `❯` prompt character.
- Focused input switches border color to `ThemeToken::BorderFocused` (`BorderType::Rounded`).
- Terminal cursor is strictly isolated to the active input line.

### C. Slash Completion & Palette Overlays
- Slash completion popup is positioned **directly below the prompt line**.
- Completion items participate in layout reflow with content reflowing upward.
- The status footer is automatically hidden while an overlay owns the lower interaction region.

### D. Document-First Workspace
- Chat timeline dominates the screen vertically.
- Sidebar uses single `│` dividers instead of heavy double borders.
- Unread divider indicator appears when new content arrives while scrolled up.

---

## 4. Boundaries (Brain Identity vs. Claude Aesthetics)

| Concern | Brain Native Identity | Claude-Inspired Design |
|---|---|---|
| Branding & Logo | Brain mascot & BRAIN identity | Warm orange color tone & whitespace rhythm |
| Domain Models | Relational Memory & Knowledge Graph | Typeface contrast & restrained headers |
| Status Footer | Daemon, Latency, Memory Context | Compact bottom-anchored single row |
| Interactive Overlays | Graph Explorer & Palette Search | Clean list reflow & isolated cursor |
