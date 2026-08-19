---
status: superseded
superseded_by: docs/design/CLAUDE_VISUAL_CONTRACT.md
date_superseded: 2026-08-14
historical_category: legacy-ui
---

# TUI Theme Token Palette Specification (HISTORICAL ARCHIVE RECORD)

> **GOVERNANCE STATUS — SUPERSEDED & RETIRED**: This specification previously defined raw hex color tokens for legacy Brain themes (including electric violet and cyberpunk palettes).
>
> **SOLE CANONICAL AUTHORITY**: All active frontend theme tokens and color values are governed exclusively by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).

---

## 1. Historical Summary
This document originally specified:
- Raw hex color mappings for legacy Brain themes.
- Accent tokens utilizing electric purple (`#6C5CE7`) and neon cyan (`#00CEC9`).

## 2. Supersession Justification
The theme token system has been standardized on the **source-grounded Claude Code color system** (`/Users/ritikpathania/Developer/src/utils/theme.ts`):
- `claude`: `rgb(215,119,87)` / `#D77757`
- `promptBorder`: `rgb(136,136,136)` / `#888888`
- `subtle`: `rgb(80,80,80)` / `#505050`
- `permission`: `rgb(177,185,249)` / `#B1B9F9`
- `autoAccept`: `rgb(175,135,255)` / `#AF87FF`

For current normative specifications, refer directly to [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).
