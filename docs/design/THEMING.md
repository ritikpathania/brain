# Theming Specification

This document defines the semantic theming architecture for the Brain TUI. Themes are built around semantic design tokens rather than hardcoded colors, ensuring that styling remains modular, contrast-compliant, and adaptable to various terminal types.

---

## 1. Semantic Design Tokens

The Brain TUI utilizes a set of semantic tokens to represent visual intent:

| Token Name | Intended Use / Meaning | Active State Example |
| :--- | :--- | :--- |
| **Primary** | Brand identifier (Warm Orange) | Focused panel borders, logo, system header |
| **Secondary** | Secondary accents and secondary panel headers | Active session selection highlight |
| **Accent** | Purple-violet indicator | Auto-approved / merged states, memory references |
| **Muted** | Low-contrast text | Timestamps, inactive borders, locked prompt text |
| **Success** | Positive confirmations | Completed plan steps, successful connections (`✓`) |
| **Warning** | Cautionary states | Quota warnings, folder trust prompts, rate limits (`!`) |
| **Danger** | Operational failures | Network disconnects, execution errors (`✗`) |
| **Thinking** | Background computation status | Spinning characters during planning/tool execution |
| **Streaming** | Live assistant text output | Typewriter rendering token characters |
| **User** | Developer message markers | "User:" sender header label |
| **Assistant** | Assistant message markers | "Assistant:" sender header label |
| **Tool** | Bash command outputs / subprocesses | Border styling for executed terminal blocks |
| **System** | Daemon notification logs | Inline system warnings and socket events |

---

## 2. Visual Invariants & Accessibility Guarantees

### 2.1. Contrast Ratios
1. **Primary Text**: Any text carrying critical workspace instructions, prompt entries, or assistant messages must maintain a contrast ratio equivalent to `4.5:1` against the terminal background.
2. **Dimmed/Muted Elements**: Secondary details (timestamps, borders, inactive widgets) must maintain a contrast ratio equivalent to `3:1`.

### 2.2. Dual-Channel State Indicators
Color must never be the sole mechanism for conveying state changes:
* **Success states** must combine green hues with text labels or icons: `[SUCCESS]` or `✓`.
* **Error states** must combine red hues with descriptive prefixes or symbols: `[ERROR]` or `✗`.
* **Warning states** must combine yellow/amber hues with symbols: `[WARNING]` or `!`.

### 2.3. Theme Resolving Hierarchy
When a theme configuration is loaded, it must map to the following fallback layers:
1. **Truecolor (24-bit RGB)**: Default mode, rendering exact RGB values matching the design token specification.
2. **256-Color Mode**: Approximates RGB values using the closest color indices in the xterm-256 color cube.
3. **ANSI-16 Mode**: Maps semantic tokens to standard ANSI system colors (e.g. Red, Green, Yellow, Blue, Magenta, Cyan, White, Black).
4. **Accessible / Monochrome Mode**: Overrides color output entirely, relying exclusively on text formatting attributes (Bold, Italic, Dim, Reverse Video) and text-based state indicators.
