---
status: active
owner: tui
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# TUI Theme Token Palette Specification

This document defines the canonical theme token palette for the Brain Terminal User Interface (TUI). All widget components in `crates/brain-tui` reference these semantic tokens rather than raw hex/RGB values.

---

## 1. Brand & Accent Tokens

| Token | RGB Value | Purpose |
|---|---|---|
| `primary` | `rgb(215, 119, 87)` | Brand accent. Headers, active tabs, focused borders. |
| `primaryShimmer` | `rgb(235, 159, 127)` | Lighter accent for animated shimmer on spinners. |
| `permissionAccent` | `rgb(156, 136, 255)` | Permission prompts, safety checks, confirmation dialogs. |

---

## 2. Text & Content Tokens

| Token | RGB Value | Purpose |
|---|---|---|
| `text` | `rgb(235, 237, 240)` | High-contrast body text. |
| `textMuted` | `rgb(120, 130, 140)` | Secondary text, timestamps, labels. |
| `textSubtle` | `rgb(80, 90, 100)` | Disabled or background text. |

---

## 3. Feedback & Status Tokens

| Token | RGB Value | Purpose |
|---|---|---|
| `success` | `rgb(46, 204, 113)` | Operations completed, passing tests, active status. |
| `warning` | `rgb(241, 196, 15)` | Warnings, non-fatal errors, reconnecting indicators. |
| `error` | `rgb(231, 76, 60)` | Errors, failed assertions, connection loss. |
| `info` | `rgb(52, 152, 219)` | Informational banners and system notices. |

---

## 4. Spacing & Typography Tokens

```yaml
spacing:
  none: 0ch
  tight: 1ch
  normal: 2ch
  relaxed: 3ch
  section: 4ch
```
