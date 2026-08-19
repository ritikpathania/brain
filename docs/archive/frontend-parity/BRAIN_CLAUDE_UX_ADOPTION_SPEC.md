# Brain TUI — Claude-Inspired Product UX Specification

## Executive Summary

This document specifies the redesign of Brain TUI's interaction model to incorporate Claude-family UX principles (contextual keyboard hints, minimal persistent chrome, strong prompt/content separation, overlays, and slash completion) while remaining 100% Brain-native in architecture, command terminology, and domain behavior.

---

## 1. Adopted, Adapted, and Rejected UX Patterns

### Adopted Patterns (From Forensic Audit)
- **Primary Landing Prompt**: Immediate readiness on Home landing surface.
- **Contextual Footer / Keyboard Hints**: Screen-specific interaction hints updated dynamically.
- **Slash Completion (`/`)**: Inline command completion popup overlay with filtering and description.
- **Command Palette (`Ctrl+K`)**: Modal search palette for commands and navigation.
- **Overlays & Dialogs**: Transient modals (`?` help, `Ctrl+X` delete confirmation, `Space` reply composer) over background content.
- **Clean Input Separation**: Dedicated prompt area with clear focus ring and mode indicator (`❯`).

### Adapted Patterns (Brain-Native)
- **Workspace Navigation**: `Left Arrow` from Home shifts focus to Workspace session browser; `Right Arrow` shifts back to Home.
- **Space vs Enter Distinction**: In Workspace, `Enter` opens the session for inspection (`SESSION_VIEW`), while `Space` opens the fast-path `REPLY_COMPOSER` modal.
- **Brain Slash Commands**: Brain-native commands (`/search`, `/context`, `/theme`, `/export`, `/status`, `/compact`) rather than copying Claude Code command names.
- **Brain Color Palette & Theme Tokens**: HSL-tailored dark and light themes with Ratatui theme tokens (no hardcoded ANSI/RGB).

### Rejected Patterns
- **No Generic Device/Window Chrome**: Restrained, clean terminal layout without excess boxes.
- **No Third-Party Branding**: Pure Brain branding, iconography, and domain terminology.
- **No Forced Mouse Dependencies**: 100% keyboard-first navigation with zero required mouse interaction.

---

## 2. Single Authoritative Navigation State Model

To prevent state drift, `UiState` + `Screen` is the **single authoritative navigation state**. Transient overlays (`REPLY_COMPOSER`, `DELETE_CONFIRMATION`, `HELP`) are represented as modal dialog state within `UiState`.

```
                         BRAIN
                           │
                    ┌──────▼──────┐
                    │     HOME    │
                    │             │
                    │ Query       │
                    │ Answer      │
                    │ Follow-up   │
                    └──────┬──────┘
                           │
                        ←  │  →
                           │
                    ┌──────▼──────┐
                    │  WORKSPACE  │
                    │             │
                    │ ↑↓ sessions │
                    └──────┬──────┘
                           │
             ┌─────────────┼──────────────┐
             │             │              │
           Enter         Space          Ctrl+X
             │             │              │
             ▼             ▼              ▼
         SESSION_VIEW   REPLY          DELETE
             │          COMPOSER       CONFIRM
             │
             └──────→ continue
```

---

## 3. Keyboard Grammar Matrix

| Surface / Context | Key | Action | Result |
|---|---|---|---|
| **Home** | `Enter` | `Action::SubmitPrompt` | Send query / create or continue session |
| **Home** | `Left Arrow` | `Action::NavigateToWorkspace` | Switch to Workspace |
| **Workspace** | `Up` / `Down` | `Action::SelectPreviousSession` / `Action::SelectNextSession` | Change session selection |
| **Workspace** | `Enter` | `Action::OpenSelectedSession` | Open session (`SESSION_VIEW`) |
| **Workspace** | `Space` | `Action::OpenReplyComposer` | Open fast-path reply composer modal |
| **Workspace** | `Ctrl+X` | `Action::OpenDeleteConfirmation` | Open delete confirmation modal |
| **Workspace** | `Right Arrow` | `Action::NavigateToHome` | Back to Home |
| **Session View** | `Enter` | `Action::SubmitPrompt` | Send query / continue conversation |
| **Session View** | `Esc` / `Right Arrow` | `Action::NavigateToWorkspace` | Back to Workspace |
| **Reply Composer** | `Enter` | `Action::SubmitReply` | Send reply |
| **Reply Composer** | `Esc` | `Action::CloseModal` | Cancel reply composer |
| **Delete Confirm** | `Enter` | `Action::ConfirmDeleteSession` | Delete session |
| **Delete Confirm** | `Esc` | `Action::CloseModal` | Cancel deletion |

---

## 4. Contextual Footer Grammar

The footer dynamically renders hints based on the active screen / modal state:

| Screen / Context | Footer Line |
|---|---|
| **HOME** | `/ Commands   Ctrl+K Palette   ← Workspace   ? Help` |
| **WORKSPACE** | `↑↓ Select   Enter Open   Space Reply   Ctrl+X Delete   → Back` |
| **SESSION_VIEW** | `Enter Send   Esc Back   ? Help` |
| **REPLY_COMPOSER** | `Enter Send   Esc Cancel` |
| **DELETE_CONFIRMATION** | `Enter Delete   Esc Cancel` |

---

## 5. Architectural & Domain Invariants
1. **Single Authoritative Navigation State**: `UiState` + `Screen` is the single source of truth for navigation.
2. **First Query Session Creation**: First query submitted on Home creates/activates a session in `UiState`/`AppState`, reusing Brain's existing persistence boundaries.
3. **Pure `Action` Reducers**: Key events in `lib.rs` emit `Action` enum variants handled by pure reducers in `state.rs`. No business logic or direct mutations in `lib.rs`.
