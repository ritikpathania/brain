# Brain TUI Landing Page Specification

> **STATUS: REFINED (v2.0)**  
> **RULE**: Home is a product landing surface, not an operational dashboard. Secondary content may occupy surrounding whitespace only when it is contextual, non-diagnostic, and subordinate to the prompt/hero (e.g. Memory Context summary on wide displays). Operational diagnostics (latency, socket stats) belong strictly in `/status` or `/health`.

---

## 1. Overview & Core Intent

The Brain TUI landing page (`Screen::Home` when `active_messages.is_empty()`) is a **product page**, not an operational dashboard. It communicates brand identity, status, and instant invitation to act (`❯`).

---

## 2. Canonical ASCII / Pixel Art Layout

```text
 BRAIN                                                                           ● Connected
────────────────────────────────────────────────────────────────────────────────────────────

                                        ▗▄▄▄▄▖
                                        █ ◉◉ █
                                        █▂▂▂▂█
                                        ▝▀▀▀▀▘
                                         BRAIN
                                Relational Memory Engine
                                         ● Ready

                             Think once. Remember forever.


Try
▶ /session new    Start a new session
  /search         Search memories
  /help           View commands

────────────────────────────────────────────────────────────────────────────────────────────
❯
────────────────────────────────────────────────────────────────────────────────────────────
```

---

## 3. Composition & Spacing Rules

1. **Header Bar (`header_h = 2`)**:
   - Single top line: Brand name `BRAIN` on the left, connection status indicator (`● Connected`) on the right.
   - Horizontal divider rule (`──────`) directly under text.
   - Zero padding between header divider rule and main content stage.

2. **Optical Hero Stage (`home_chunks[0]`)**:
   - Centered alignment (`Alignment::Center`).
   - Unified logo mark:
     - 4-line static Memory Core pixel art (`▗▄▄▄▄▖ / █ ◉◉ █ / █▂▂▂▂█ / ▝▀▀▀▀▘`).
     - **BRAIN** header (`HeaderPrimary`, Bold).
     - **Relational Memory Engine** subtitle (`TextSecondary`).
     - State indicator (`● Ready` / `◐ Searching` / `◓ Reasoning` / `✔ Memory updated` / `○ Disconnected`) immediately beneath subtitle with no blank line gap.

3. **Brand Tagline Stage (`home_chunks[1]`)**:
   - Single clean tagline: *"Think once. Remember forever."*
   - Centered alignment (`Alignment::Center`).
   - No explicit operational instructions (e.g. no "Press / to begin" or "Connected to UDS").

4. **Lightweight Launcher (`home_chunks[2]`)**:
   - Two-column layout (`▶ /command   description`).
   - "Try" section header (`TextSecondary`, Bold).
   - Maximum 3 primary commands: `/session new`, `/search`, `/help`.
   - Responsive truncation: description column hides if panel width < 48 chars.

5. **Prompt Bar Destination (`prompt_h = 3`)**:
   - `❯` prompt character in accent color.
   - When focused with empty text: suppresses placeholder text (`Ask a question...`), allowing the terminal cursor to be the sole visual destination cue.

---

## 4. Explicit Non-Goals

The landing page MUST NOT display:
- Memory statistics or entity counts
- Recent session lists
- System operational diagnostics or latency graphs
- Reasoning history or activity feeds
- Telemetry metrics
- Product news or tips of the day
- Installed plugin lists
- Active workspace path metrics

Those concerns belong strictly in dedicated views (`/memory`, `/status`, `/health`, `/session`).

---

## 5. Invariant Checklist for Code Reviews

- [x] No side-by-side diagnostic cards on startup (System Status box removed from Home landing).
- [x] Mascot silhouette remains static; state is expressed purely via the status indicator.
- [x] Sidebar (`sb_w`) collapses to width 0 on Welcome screen and expands (`sb_w = 25`) only in Workspace screen.
- [x] Terminal native background (`Color::Reset`) is preserved across all theme palettes.
