# Phase 5.2 Forensic UI Audit: Landing & Empty State Specification

> **Document Status**: Forensic Audit & Component Specification (Complete — Pre-Implementation)  
> **Target Subsystem**: `packages/brain-frontend` (`src/components/WelcomeHero.tsx`, `src/components/Messages.tsx`)  
> **Canonical Design Authority**: [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) (Section 6) & [`docs/design/CLAUDE_COMPONENT_MODEL.md`](./CLAUDE_COMPONENT_MODEL.md) (Primitive 4: `LogoHeader`)  
> **Governance Note on `LANDING_PAGE.md`**: Verified as superseded historical archive; canonical authority is `CLAUDE_VISUAL_CONTRACT.md`.  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
PHASE 5.2 FORENSIC UI AUDIT: LANDING & EMPTY STATE
================================================================================
CURRENT RENDER PATH: App.tsx -> FullscreenLayout.tsx -> Messages.tsx (lines 19-30)
CURRENT VISUAL: Simple centered text "Welcome to Claude Code (Brain Relational Memory)"
TARGET COMPONENT: WelcomeHero.tsx (LogoHeader / Greeting Panel)
DESIGN GRAMMAR:
  - Left Panel: Brain brand, "Relational Memory Engine", "Think once. Remember forever.", live status
  - Right Panel: Getting Started / Launcher hints (/help, /sessions, Ctrl+K, /status)
  - Layout: Clean two-panel responsive split on native terminal floor
BACKEND & PROTOCOL MODIFICATIONS: ZERO (0 lines changed)
PRESENTATION STATE CONTRACT: 100% Intact & Unchanged
VERDICT: PROCEED TO PHASE 5.2 IMPLEMENTATION
================================================================================
```

---

## 1. Audit of Existing Empty-State Render Path

In `packages/brain-frontend/src/components/Messages.tsx`:
```tsx
// Current lines 19-30 in Messages.tsx
if (messages.length === 0 && !isStreaming) {
  return (
    <Box flexDirection="column" alignItems="center" justifyContent="center" marginY={2}>
      <Text color="cyan" bold>
        Welcome to Claude Code (Brain Relational Memory)
      </Text>
      <Text color="gray">
        Type your query below or use / for commands (e.g. /help, /config)
      </Text>
    </Box>
  );
}
```

**Identified Shortcomings**:
1. Lacks structured typographic hierarchy and distinct two-panel layout.
2. Uses placeholder prototype title `"Welcome to Claude Code (Brain Relational Memory)"` rather than Brain's actual product identity.
3. Lacks actionable launcher hints (`/help`, `/sessions`, `Ctrl+K`, `/status`).
4. Lacks the canonical Brain tagline *"Think once. Remember forever."*.

---

## 2. Canonical Specification: `WelcomeHero.tsx` (`LogoHeader`)

Derived directly from `CLAUDE_VISUAL_CONTRACT.md` (Section 6) and `CLAUDE_COMPONENT_MODEL.md` (Primitive 4: `LogoHeader`):

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ BRAIN v1.1.0                             │ Getting Started                  │
│ Relational Memory Engine                 │ ▶ /help      View commands       │
│                                          │ ▶ /sessions  Switch sessions     │
│ "Think once. Remember forever."          │ ▶ Ctrl+K     Search memory graph │
│ ● daemon:connected                       │ ▶ /status    Engine diagnostics  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Props:
```tsx
export interface WelcomeHeroProps {
  engineVersion?: string;
  daemonStatus?: 'connected' | 'connecting' | 'disconnected';
  memoryStatus?: string;
}
```

### Layout Rules:
1. **Native Terminal Floor**: No colored solid rectangular fills.
2. **Left Column**:
   - Primary Brand Headline: `BRAIN v1.1.0` (`ThemeTokens.colors.accent`, bold).
   - Engine Subtitle: `Relational Memory Engine` (`ThemeTokens.colors.textSecondary`).
   - Tagline: `Think once. Remember forever.` (`ThemeTokens.colors.brandGold`, italic/dim).
   - Status Pill: `● daemon:connected │ memory:active` (`ThemeTokens.colors.statusConnected`).
3. **Right Column (`Getting Started` / `Launcher Hints`)**:
   - Section Header: `Getting Started` (`ThemeTokens.colors.textSecondary`, bold).
   - Launcher items with pointer glyph `▶ `:
     - `▶ /help` — Show available slash commands & reference
     - `▶ /sessions` — List and switch conversation sessions
     - `▶ Ctrl+K` — Search relational memory graph & command palette
     - `▶ /status` — View background daemon & projection health
4. **Behavioral Invariant**:
   - Renders only when `messages.length === 0 && !isStreaming`.
   - Once the first user prompt is submitted, the conversation stream replaces the welcome hero.

---

## 3. Integration & Testing Plan

1. Create `packages/brain-frontend/src/components/WelcomeHero.tsx`.
2. Update `packages/brain-frontend/src/components/Messages.tsx` to mount `<WelcomeHero />` in the empty state.
3. Update `packages/brain-frontend/src/components/App.tsx` (passes daemon/memory status to `Messages`).
4. Run regression tests (`bun test`) ensuring all 102 tests pass (including `fixtures.test.ts` fixture `FIX-01 empty_landing`).
5. Verify Rust workspace (`cargo check`).

---

## 4. Final Audit Verdict

```text
================================================================================
AUDIT VERDICT:
PASS — READY FOR PHASE 5.2 IMPLEMENTATION
================================================================================
```
