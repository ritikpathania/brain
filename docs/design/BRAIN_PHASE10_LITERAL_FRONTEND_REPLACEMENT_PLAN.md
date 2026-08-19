# Phase 10 — Literal Claude Frontend Replacement Architectural Plan

> **Document Status**: Approved Architecture & Migration Specification  
> **Oracle Ground Truth**: Source inspection of `/Users/ritikpathania/Developer/src` (114 React 18 + Ink 5 + Yoga components)  
> **Target Subsystem**: `packages/brain-frontend` (React 18 + Ink 5 + Yoga under Bun)  
> **Integration Boundary**: `BrainFrontendController` → `BrainFrontendAdapter` → `BrainUdsClient` → `Brain Rust Daemon` (100% UNCHANGED)  
> **Acceptance Standard**: `LITERAL CLAUDE FRONTEND REPLACEMENT`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
PHASE 10 — LITERAL CLAUDE FRONTEND REPLACEMENT ARCHITECTURE
================================================================================
TARGET ARCHITECTURE:

                     CLAUDE CODE FRONTEND
          (Exact components, layout, props, hooks, styles)
                             │
                             ▼ (Claude presentation props)
                    BrainFrontendAdapter
          (Translates Brain state into Claude component models)
                             │
                             ▼
                    BrainFrontendController
                             │
                             ▼
                      BrainUdsClient
                             │
                             ▼
                     Brain Rust Daemon
================================================================================
```

---

## 1. Direct Component Replacement & Naming Alignment

All Brain-authored parallel components are replaced with Claude Code's exact component names, contracts, and hierarchies:

| Current Reconstructed File | Canonical Claude Source Component | Replacement Target File | Component Contract / Props |
|---|---|---|---|
| `components/LogoHeader.tsx` | `components/LogoV2/LogoV2.tsx` + `CondensedLogo.tsx` | `components/LogoV2.tsx` | `LogoV2Props: { version, cwd, model, tagline, feedItems }` |
| `components/BaseTextInput.tsx` | `components/PromptInput/PromptInput.tsx` | `components/PromptInput.tsx` | `PromptInputProps: { value, cursorOffset, placeholder, focused, multiline }` |
| `components/SlashAutocompletePopup.tsx` | `components/design-system/FuzzyPicker.tsx` | `components/FuzzyPicker.tsx` | `FuzzyPickerProps: { items, selectedIndex, filterText }` |
| `components/ShortcutsHelpModal.tsx` | `components/HelpV2/HelpV2.tsx` | `components/HelpV2.tsx` | `HelpV2Props: { sections }` |
| `components/messages/MarkdownText.tsx` | `components/Markdown.tsx` + `HighlightedCode.tsx` | `components/Markdown.tsx` | `MarkdownProps: { content, syntaxHighlight }` |
| `components/messages/UserTextMessage.tsx` | `components/messages/UserPromptMessage.tsx` | `components/messages/UserPromptMessage.tsx` | `UserPromptMessageProps: { content, isTranscriptMode }` |
| `components/messages/RecalledMemoryChip.tsx` | `components/messages/UserMemoryInputMessage.tsx` | `components/messages/UserMemoryInputMessage.tsx` | `UserMemoryInputMessageProps: { memoryIds, text }` |
| `components/FullscreenLayout.tsx` | `components/FullscreenLayout.tsx` | `components/FullscreenLayout.tsx` | `FullscreenLayoutProps: { scrollable, bottom, modal, stickyPrompt }` |
| `components/Messages.tsx` | `components/Messages.tsx` | `components/Messages.tsx` | `MessagesProps: { messages, streamingText, isStreaming, unseenDividerIndex }` |
| `components/MessageRow.tsx` | `components/MessageRow.tsx` | `components/MessageRow.tsx` | `MessageRowProps: { message, isStreaming }` |
| `components/StatusLine.tsx` | `components/StatusLine.tsx` | `components/StatusLine.tsx` | `StatusLineProps: { cwd, statusText, shortcutHints }` |
| `components/GlobalSearchDialog.tsx` | `components/GlobalSearchDialog.tsx` | `components/GlobalSearchDialog.tsx` | `GlobalSearchDialogProps: { searchQuery, items, selectedIndex }` |

---

## 2. Strict Semantic Adapter Layer

Brain-specific capabilities are translated strictly at the `BrainFrontendAdapter` boundary into Claude-shaped presentation data models. Zero Brain-specific UI logic exists in the presentation components:

```typescript
// BrainFrontendAdapter.ts: Presentation Translation Boundary

export interface LogoV2Props {
  version: string;
  cwd: string;
  model: string;
  tagline: string;
  feedItems: Array<{ key: string; title: string; subtitle: string }>;
}

export interface StatusLineProps {
  cwd: string;
  statusText: string;
  shortcutHints: string[];
}

export interface FuzzyPickerProps {
  items: Array<{ name: string; description: string; category?: string }>;
  selectedIndex: number;
  filterText: string;
}

export interface HelpV2Section {
  title: string;
  shortcuts: Array<{ key: string; description: string }>;
}

export class BrainFrontendAdapter {
  // ...
  public getLogoV2Props(): LogoV2Props {
    return {
      version: this.state.footer.engineVersion || '1.1.0',
      cwd: this.state.session.workingDirectory || process.cwd(),
      model: `Brain (${this.state.footer.daemonStatus}) · Memory (${this.state.footer.memoryStatus})`,
      tagline: 'Think once. Remember forever.',
      feedItems: [
        { key: '/help', title: 'Help & Docs', subtitle: 'View command reference' },
        { key: '/sessions', title: 'Sessions', subtitle: 'Switch active workspace session' },
        { key: 'Ctrl+K', title: 'Command Palette', subtitle: 'Search memory & execute tools' },
        { key: '/status', title: 'System Status', subtitle: 'Check daemon & metrics' },
      ],
    };
  }

  public getStatusLineProps(): StatusLineProps {
    return {
      cwd: this.state.session.workingDirectory || process.cwd(),
      statusText: `v${this.state.footer.engineVersion} · ${this.state.footer.daemonStatus} · ${this.state.footer.memoryStatus}`,
      shortcutHints: ['Ctrl+K Commands', 'Ctrl+O Expand', 'Esc Dismiss'],
    };
  }

  public getFuzzyPickerProps(selectedIndex: number = 0): FuzzyPickerProps {
    return {
      items: [
        { name: '/reflect', description: 'Run memory consolidation & reflection' },
        { name: '/compile', description: 'Compile observation buffers into knowledge graph' },
        { name: '/inspect', description: 'Inspect memory graph node relationships' },
        { name: '/sessions', description: 'List and switch active workspace sessions' },
        { name: '/status', description: 'Display background daemon and memory health' },
        { name: '/diagnostics', description: 'Run complete system diagnostic harness' },
        { name: '/capabilities', description: 'Query active capabilities and tool registry' },
        { name: '/rebuild', description: 'Rebuild relational memory and hybrid index' },
        { name: '/help', description: 'Show keyboard shortcuts and command manual' },
        { name: '/clear', description: 'Clear active terminal scrollback' },
        { name: '/exit', description: 'Close session and shutdown frontend client' },
      ],
      selectedIndex,
      filterText: this.state.prompt.buffer,
    };
  }
}
```

---

## 3. Migration Roadmap

1. **Step 1 — Component Extraction & Replacement**:
   - Extract and replace `LogoHeader` with `LogoV2` (`LogoV2.tsx` + `CondensedLogo.tsx`).
   - Extract and replace `BaseTextInput` with `PromptInput` (`PromptInput.tsx` + `PromptInputFooter.tsx`).
   - Extract and replace `SlashAutocompletePopup` with `FuzzyPicker` (`FuzzyPicker.tsx`).
   - Extract and replace `ShortcutsHelpModal` with `HelpV2` (`HelpV2.tsx`).
   - Extract and replace `MarkdownText` with `Markdown` (`Markdown.tsx` + `HighlightedCode.tsx`).
   - Extract and replace `UserTextMessage` with `UserPromptMessage` (`UserPromptMessage.tsx`).
   - Extract and replace `RecalledMemoryChip` with `UserMemoryInputMessage` (`UserMemoryInputMessage.tsx`).
2. **Step 2 — Root Shell Integration**:
   - Update `App.tsx`, `FullscreenLayout.tsx`, `Messages.tsx`, and `MessageRow.tsx` to mount the canonical Claude components.
3. **Step 3 — Test & Verification**:
   - Run `bun test` across full test suite.
   - Run `cargo check` on backend crates.
   - Run multi-viewport acceptance testing (`80x24`, `100x30`, `120x40`, `182x53`).

---

## 4. Phase 10 Acceptance Gate

```text
================================================================================
PHASE 10 ACCEPTANCE GATE
================================================================================
[ ] Claude component source reused directly (LogoV2, PromptInput, FuzzyPicker, HelpV2, Markdown)
[ ] Zero parallel Brain implementations in components/
[ ] Brain-specific data strictly translated in BrainFrontendAdapter
[ ] Rust backend & UDS protocol 100% UNCHANGED (0 lines modified)
[ ] 153+ automated tests PASS
[ ] Multi-viewport acceptance PASS across 80x24, 100x30, 120x40, 182x53
================================================================================
TARGET VERDICT: LITERAL CLAUDE FRONTEND REPLACEMENT CERTIFIED 🔒
================================================================================
```
