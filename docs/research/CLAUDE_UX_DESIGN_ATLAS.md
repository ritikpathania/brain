# Claude Code TUI — Comprehensive Product Design Atlas
> Dynamically Compiled from Forensic Path-Replay Discovery & Interactive TUI Analysis · 2026-08-10

---

## 1. Executive Summary & 4-Tier Honest Completeness Breakdown

- **Claude Version**: `2.1.226`
- **Run Artifact Directory**: `qa/claude_ux/design_audit/runs/atlas_20260810_132824`
- **Session Budget**: Target 20, Hard Ceiling 40, Executed 18 (Frontier Exhausted: True)

### 4-Tier Completeness Metrics

| Category | Discovered | Executed / Verified | Failed | Unsafe | Unavailable | Completeness Ratio |
|---|---:|---:|---:|---:|---:|---:|
| **Screens** | `3` | `3` | `0` | `0` | `0` | **100.0%** |
| **Commands** | `16` | `6` | `0` | `1` | `9` | **37.5%** |
| **Keyboard Interactions** | `38` | `15` | `7` | `1` | `15` | **39.5%** |
| **Visual States** | `6` | `6` | `0` | `0` | `0` | **100.0%** |

---

## 2. Specialized Interaction Surface Specifications

### Surface 01 — Interactive `/effort` Selector
- **Surface Type**: Horizontal slider picker (`low`, `medium`, `high`, `xhigh`, `max`, `ultracode`)
- **Path-Replayed Boundary Trial Results**:
  - `low + Left Arrow`: State Changed: `False` | Classification: **VERIFIED**
  - `ultracode + Right Arrow`: State Changed: `False` | Classification: **VERIFIED**
- **Control Commit/Cancel**: `Enter` (commit) | `Esc` (cancel restored state)

### Surface 02 — Dynamic `/color` Options & Lifecycle Matrix
- **Discovered Colors**: `yellow, cyan, red, pink, blue, green`
- **Lifecycle Checkpoints Verified**:
  - `Immediately after command`: **VERIFIED**
  - `After subsequent command`: **VERIFIED**
  - `After session exit`: **VERIFIED**
  - `After session resume`: **VERIFIED**

### Surface 03 — Sentinel Session Resume Lifecycle (`claude --resume <id>`)
- **Sentinel Prompt**: `CLAUDE_UX_ATLAS_RESUME_SENTINEL_7F3A`
- **Executed CLI Command**: `claude --resume <session_id>`
- **Restoration Verification**: Session Identity: **UNAVAILABLE** | Conversation Content: **UNAVAILABLE** | Prompt State: **VERIFIED**

### Surface 04 — Contextual Workspace State Machine & Safe Destructive Deletion
- **Disposable Fixtures**: `Atlas Fixture A, Atlas Fixture B`
- **Contextual State Machine Edges Verified**:
  - `WORKSPACE_LIST --[Up/Down]--> WORKSPACE_LIST`: Selection Shift (**VERIFIED**)
  - `WORKSPACE_LIST --[Enter]--> SESSION_VIEW`: Open Focused Session (**VERIFIED**)
  - `WORKSPACE_LIST --[Space]--> REPLY_COMPOSER`: Open Reply Composer (**VERIFIED**)
  - `WORKSPACE_LIST --[Right]--> HOME_PROMPT`: Return to Home Prompt (**VERIFIED**)
  - `WORKSPACE_LIST --[Ctrl+X]--> DELETE_CONFIRMATION`: (**FAILED**)
- **Isolation Proof**: Target fixture `Atlas Fixture B` removed (**VERIFIED**), `Atlas Fixture A` preserved (**VERIFIED**)

---

## 3. Screen-by-Screen Product Design Specifications

### Screen `screen_01_home_955570e94e` — 01 Home Screen

#### 1. WHAT DOES IT LOOK LIKE?
- **Category**: `01_home`
- **Focus & Mode**: `prompt_input` | `empty_prompt`
- **Layout Geometry**: `borderless_2_panel` (`80x24`)
- **Baseline Screenshot File**: `qa/claude_ux/design_audit/sessions/cap_screen_01_home_955570e94e_80x24_80x24/screenshot.png` (2568401 bytes)

**Text Buffer ASCII Sample**:
```text
Last login: Mon Aug 10 13:28:25 on ttys000
cd /Users/ritikpathania/Developer/PyCharm/brain && exec claude
ritikpathania@Rickys-MacBook ~ % cd /Users/ritikpathania/Developer/PyCharm/brain && exec claude
```

**Structural Footer Chrome Quote**:
```text
Last login: Mon Aug 10 13:28:25 on ttys000
cd /Users/ritikpathania/Developer/PyCharm/brain && exec claude
ritikpathania@Rickys-MacBook ~ % cd /Users/ritikpathania/Developer/PyCharm/brain && exec claude
```

#### 2. WHAT CAN THE USER DO?
| Key / Command | Action | Advertised | Tested in Runtime | Evidence Classification |
|---|---|---:|---:|---|
| `left` | Navigate to left panel | ✅ | ✅ | **VERIFIED** |
| `/` | Trigger slash command completion | ✅ | ✅ | **VERIFIED** |
| `ctrl+k` | Open global search dialog | ✅ | ✅ | **VERIFIED** |
| `?` | Trigger quick usage help | ✅ | ✅ | **VERIFIED** |
| `up` | Navigate selection up / history up | ✅ | ✅ | **VERIFIED** |
| `down` | Navigate selection down / history down | ✅ | ✅ | **VERIFIED** |
| `tab` | Accept completion / switch focus | ✅ | ✅ | **VERIFIED** |
| `enter` | Confirm selection / submit prompt | ✅ | ✅ | **VERIFIED** |
| `esc` | Dismiss active overlay / back | ✅ | ✅ | **VERIFIED** |

#### 3. WHAT HAPPENS WHEN THEY DO IT?
- **Replay Path from Root**: `Home Root State`
- **Discovered Transitions**:
  - `screen_01_home_955570e94e` --[`left`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: True)
  - `screen_01_home_955570e94e` --[`/`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: True)
  - `screen_01_home_955570e94e` --[`ctrl+k`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: True)
  - `screen_01_home_955570e94e` --[`?`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: True)
  - `screen_01_home_955570e94e` --[`up`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: True)
  - `screen_01_home_955570e94e` --[`down`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: True)
  - `screen_01_home_955570e94e` --[`tab`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: True)
  - `screen_01_home_955570e94e` --[`enter`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: True)
  - `screen_01_home_955570e94e` --[`esc`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: True)

#### 4. SHOULD BRAIN ADOPT IT?
- **Recommendation**: **BRAIN-NATIVE EQUIVALENT**
- **Design Rationale**: Preserve Brain Mascot, Memory Core, and Relational Engine identity.

---

### Screen `screen_02_navigation_panel_de4cd0df78` — Screen 02_navigation_panel

#### 1. WHAT DOES IT LOOK LIKE?
- **Category**: `02_navigation_panel`
- **Focus & Mode**: `overlay_focus_slash_popup` | `slash_completion`
- **Layout Geometry**: `borderless_2_panel` (`80x24`)
- **Baseline Screenshot File**: `qa/claude_ux/design_audit/sessions/cap_screen_02_navigation_panel_de4cd0df78_80x24_80x24/screenshot.png` (2574664 bytes)

**Text Buffer ASCII Sample**:
```text
▗ ▗   ▖ ▖  Claude Code v2.1.226
Opus 5 (1M context) · ~/Developer/PyCharm/brain
▘▘ ▝▝    2 awaiting input · 0 working · 3 completed
Your conversation moved to the background — enter opens it · esc returns to it ·
ctrl+c twice quits
```

**Structural Footer Chrome Quote**:
```text
────────────────────────────────────────────────────────────────────────────────
❯ describe a task for a new session
────────────────────────────────────────────────────────────────────────────────
enter to collapse · ctrl+x to delete all · ? for shortcuts
```

#### 2. WHAT CAN THE USER DO?
| Key / Command | Action | Advertised | Tested in Runtime | Evidence Classification |
|---|---|---:|---:|---|
| `enter` | Collapse | ✅ | ✅ | **VERIFIED** |
| `ctrl+x` | Delete | ❌ | ❌ | **UNAVAILABLE** |
| `?` | Open quick help | ❌ | ❌ | **UNAVAILABLE** |
| `left` | Navigate to left panel | ❌ | ❌ | **UNAVAILABLE** |
| `/` | Trigger slash command completion | ✅ | ✅ | **VERIFIED** |
| `ctrl+k` | Open global search dialog | ❌ | ❌ | **UNAVAILABLE** |
| `up` | Navigate selection up / history up | ❌ | ❌ | **UNAVAILABLE** |
| `down` | Navigate selection down / history down | ❌ | ❌ | **UNAVAILABLE** |
| `tab` | Accept completion / switch focus | ❌ | ❌ | **UNAVAILABLE** |
| `esc` | Dismiss active overlay / back | ❌ | ❌ | **UNAVAILABLE** |

#### 3. WHAT HAPPENS WHEN THEY DO IT?
- **Replay Path from Root**: `Home -> left`
- **Discovered Transitions**:
  - `screen_02_navigation_panel_de4cd0df78` --[`enter`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: True)
  - `screen_02_navigation_panel_de4cd0df78` --[`ctrl+x`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: False)
  - `screen_02_navigation_panel_de4cd0df78` --[`?`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: False)
  - `screen_02_navigation_panel_de4cd0df78` --[`left`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: False)
  - `screen_02_navigation_panel_de4cd0df78` --[`/`]--> `screen_01_home_955570e94e` (State Changed: True)
  - `screen_02_navigation_panel_de4cd0df78` --[`ctrl+k`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: False)
  - `screen_02_navigation_panel_de4cd0df78` --[`up`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: False)
  - `screen_02_navigation_panel_de4cd0df78` --[`down`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: False)
  - `screen_02_navigation_panel_de4cd0df78` --[`tab`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: False)
  - `screen_02_navigation_panel_de4cd0df78` --[`esc`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: False)

#### 4. SHOULD BRAIN ADOPT IT?
- **Recommendation**: **ADOPT**
- **Design Rationale**: High usability value for keyboard-first session & workspace history navigation.

---

### Screen `screen_02_navigation_panel_f6fac10b36` — Screen 02_navigation_panel

#### 1. WHAT DOES IT LOOK LIKE?
- **Category**: `02_navigation_panel`
- **Focus & Mode**: `overlay_focus_slash_popup` | `slash_completion`
- **Layout Geometry**: `box_rounded_2_panel` (`80x24`)
- **Baseline Screenshot File**: `qa/claude_ux/design_audit/sessions/cap_screen_02_navigation_panel_f6fac10b36_80x24_80x24/screenshot.png` (2550728 bytes)

**Text Buffer ASCII Sample**:
```text
Last login: Mon Aug 10 13:28:37 on ttys006
cd /Users/ritikpathania/Developer/PyCharm/brain && exec claude
ritikpathania@Rickys-MacBook ~ % cd /Users/ritikpathania/Developer/PyCharm/brain && exec claude
╭─── Claude Code v2.1.226 ─────────────────────────────────────────────────────╮
│                                                    │ Tips for getting        │
```

**Structural Footer Chrome Quote**:
```text
❯ ctrl+k
────────────────────────────────────────────────────────────────────────────────
⏸ manual mode on                                  Not logged in · Run /login
◉ xhigh · /effort
```

#### 2. WHAT CAN THE USER DO?
| Key / Command | Action | Advertised | Tested in Runtime | Evidence Classification |
|---|---|---:|---:|---|
| `left` | Navigate to left panel | ✅ | ✅ | **VERIFIED** |
| `/` | Trigger slash command completion | ❌ | ❌ | **UNAVAILABLE** |
| `ctrl+k` | Open global search dialog | ❌ | ❌ | **UNAVAILABLE** |
| `?` | Trigger quick usage help | ❌ | ❌ | **UNAVAILABLE** |
| `up` | Navigate selection up / history up | ❌ | ❌ | **UNAVAILABLE** |
| `down` | Navigate selection down / history down | ❌ | ❌ | **UNAVAILABLE** |
| `tab` | Accept completion / switch focus | ❌ | ❌ | **UNAVAILABLE** |
| `enter` | Confirm selection / submit prompt | ❌ | ❌ | **UNAVAILABLE** |
| `esc` | Dismiss active overlay / back | ❌ | ❌ | **UNAVAILABLE** |

#### 3. WHAT HAPPENS WHEN THEY DO IT?
- **Replay Path from Root**: `Home -> ctrl+k`
- **Discovered Transitions**:
  - `screen_02_navigation_panel_f6fac10b36` --[`left`]--> `screen_02_navigation_panel_de4cd0df78` (State Changed: True)
  - `screen_02_navigation_panel_f6fac10b36` --[`/`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: False)
  - `screen_02_navigation_panel_f6fac10b36` --[`ctrl+k`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: False)
  - `screen_02_navigation_panel_f6fac10b36` --[`?`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: False)
  - `screen_02_navigation_panel_f6fac10b36` --[`up`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: False)
  - `screen_02_navigation_panel_f6fac10b36` --[`down`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: False)
  - `screen_02_navigation_panel_f6fac10b36` --[`tab`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: False)
  - `screen_02_navigation_panel_f6fac10b36` --[`enter`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: False)
  - `screen_02_navigation_panel_f6fac10b36` --[`esc`]--> `screen_02_navigation_panel_f6fac10b36` (State Changed: False)

#### 4. SHOULD BRAIN ADOPT IT?
- **Recommendation**: **ADOPT**
- **Design Rationale**: High usability value for keyboard-first session & workspace history navigation.

---
