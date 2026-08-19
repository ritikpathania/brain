---
status: superseded
superseded_by: docs/design/CLAUDE_VISUAL_CONTRACT.md
date_superseded: 2026-08-14
historical_category: legacy-ui
---

# Brain TUI Landing Page Specification (HISTORICAL ARCHIVE RECORD)

> **GOVERNANCE STATUS — SUPERSEDED & RETIRED**: This specification previously defined a legacy Brain-specific landing page featuring a static Memory Core pixel-art mascot and fixed telemetry dashboard cards.
>
> **SOLE CANONICAL AUTHORITY**: All active frontend visual, interaction, and layout contracts are governed exclusively by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) and [`docs/design/CLAUDE_COMPONENT_MODEL.md`](./CLAUDE_COMPONENT_MODEL.md).

---

## 1. Historical Summary
This document originally specified:
- A 4-line static ASCII pixel-art logo mark.
- A promotional tagline ("Think once. Remember forever.").
- Fixed telemetry cards ("Connected: Yes | Ready: Yes").
- 3 fixed command pills (`/session new`, `/search`, `/help`).

## 2. Supersession Justification
The product frontend direction transitioned to faithfully reproduce the **Claude Code visual language and interaction architecture**. Under the canonical contract:
- The landing page is replaced by a **scrollable typographic greeting header** (`LogoV2` / `LogoHeader`) rendered at the top of the message transcript.
- The interface adopts a **prompt-first interaction model** with an auto-expanding prompt composer at the bottom.
- Zero static telemetry boxes, pixel art, or mascot avatars exist in the active design system.

For current normative specifications, refer directly to [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).
