# UX Principles

> **AUTHORITY NOTICE**: This document is a **supporting engineering specification** for `crates/brain-tui`, strictly subordinate to and governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).


This document defines the 7 core design principles that guide the Brain Terminal User Interface (TUI) client experience. These principles ensure that Brain remains fast, intuitive, accessible, and robust.

---

## Principle 1: The Chat is Primary
The conversational stream represents the cognitive feedback loop of the assistant. It is the primary visual and structural element of the TUI.
* **Guidance**: Sidebars, tool detail boxes, help overlays, and system stats must remain visually subordinate. They must never crowd, obscure, or distract from the core chat timeline.

## Principle 2: Typing is Sacred
The input buffer belongs exclusively to the user. 
* **Guidance**: Focus must never be forcibly stolen from the input prompt during active typing. Network packets, background tasks, tool executions, or streaming completions are strictly forbidden from altering the user's current cursor position or active buffer text.

## Principle 3: Streaming Never Jumps
Streaming text updates must flow smoothly.
* **Guidance**: Viewport positioning must be stable and deterministic. The scroll container must anchor to the bottom of the content during active generation unless the user has explicitly scrolled up. Auto-scrolling must be linear and predictable, never jumping or skipping lines.

## Principle 4: Keyboard First
Keyboard control is mandatory; mouse support is optional.
* **Guidance**: Every button, session menu item, permission confirmation, dialog box, and page layout transition must be fully controllable using clear, standard, and single-level hotkeys.

## Principle 5: Every Operation Has Visible Feedback
No state should ever appear frozen.
* **Guidance**: Whenever a long-running, background, or remote operation (such as file indexing, vector search, or token streaming) is active, the interface must display continuous, non-blocking visual feedback (e.g. dynamic spinners, status lines).

## Principle 6: Errors are Recoverable
No system failure should result in a terminal dead-end or loss of state.
* **Guidance**: Errors must preserve the user's prompt input buffer so they do not have to retype their query. All failure messages must provide suggestions, status summaries, or retry shortcuts.

## Principle 7: Progressive Disclosure
Keep the default screen uncluttered.
* **Guidance**: Advanced telemetry metrics, raw JSON socket traffic, memory engine parameters, and detailed logs are tucked away behind specialized modes, palettes, or diagnostics displays. They should only reveal themselves upon explicit command.
