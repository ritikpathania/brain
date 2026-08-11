# Claude UX Research Harness — Environment Discovery
> Captured: 2026-08-10 · Host OS: macOS (Darwin arm64)

---

## 1. Binary & Executable Discovery

| Property | Value | Source |
|---|---|---|
| Claude Executable Path | `/Users/ritikpathania/.local/bin/claude` | `which claude` |
| Claude Version | `2.1.226 (Claude Code)` | `claude --version` |
| Claude Source Path | `/Users/ritikpathania/Developer/src` | Local filesystem |
| Environment runtime | Bun / Node.js | Source inspect |

**[OBSERVED]**: Binary is executable at `/Users/ritikpathania/.local/bin/claude` and responds to `--version` and `--help`.

---

## 2. macOS Terminal Applications & Automation Capability

| Tool / App | Availability | Automation Protocol | Status |
|---|---|---|---|
| `osascript` | `/usr/bin/osascript` | AppleScript | Active & functional |
| `screencapture` | `/usr/sbin/screencapture` | macOS CLI | Active & functional |
| `Terminal.app` | Installed | AppleScript (`tell application "Terminal"`) | Supported |
| `iTerm2` | Installed (`iTerm`) | AppleScript (`tell application "iTerm"`) | Supported |
| Native Vision OCR | `qa/applescript/ocr` | Apple Vision API (Swift) | Active & functional (0.05s response) |
| Python 3 | `/usr/bin/python3` | Subprocess / Standard Lib | Active |
| Swift Compiler | `/usr/bin/swiftc` | macOS SDK (v6.4) | Active |

**[OBSERVED]**: Both `Terminal.app` and `iTerm2` are installed and AppleScript scriptable. The native Swift Vision OCR helper at `qa/applescript/ocr` executes text recognition accurately.

---

## 3. Claude CLI Deterministic Testing Flags

From `claude --help` inspection [SOURCE-CONFIRMED]:

| Flag | Purpose in Harness |
|---|---|
| `-p`, `--print` | Non-interactive output mode |
| `--add-dir <dirs>` | Restrict file access scope |
| `--agent <agent>` | Specify agent mode |
| `--allowedTools <tools>` | Pre-approve tools for deterministic runs |
| `--append-system-prompt` | System prompt isolation |
| `process.env.CLAUDE_CODE_FORCE_FULL_LOGO` | Forces full LogoV2 on startup |
| `process.env.IS_DEMO` | Demo mode toggle |

---

## 4. Test & Demo Fixtures in Source

From source code inspection at `/Users/ritikpathania/Developer/src` [SOURCE-CONFIRMED]:

- `src/components/LogoV2/feedConfigs.ts`: Contains predefined onboarding feeds and release notes fixtures.
- `src/keybindings/defaultBindings.ts`: Authoritative keybinding map.
- `src/utils/logoV2Utils.ts`: Precise layout breakpoints (≥70 cols horizontal mode, left panel max 50 cols).
- `src/components/PromptInput/PromptInput.tsx`: Input constants (`PROMPT_FOOTER_LINES = 5`, `MIN_INPUT_VIEWPORT_LINES = 3`).
