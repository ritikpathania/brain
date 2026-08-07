# Cross-Terminal Compatibility & Capability Matrix Report

**Environment**: macOS Darwin arm64  
**Date**: August 07, 2026  
**Status**: All Required and Optional Compatibility Gates PASSED  

---

## 1. Overview & Capability Levels

The `brain-tui` client is designed to support a wide range of terminal emulators while maintaining predictable visual styling, keyboard event routing, and fallback semantics.

Capabilities are categorized into two strict levels:
- **Required**: Critical visual or functional invariants (e.g., border rendering integrity, color degradation handling under `NO_COLOR`, modifier key routing). Failure of a Required capability blocks release.
- **Optional**: Non-essential enhancements (e.g., OSC8 hyperlink escape sequences, native OS clipboard integration). Supported terminals utilize these features; unsupported terminals fall back gracefully without degrading the user experience.

---

## 2. Terminal Compatibility Matrix across Target Profiles

| Terminal Profile | Validation Status | Feature Capability | Level | Expected | Observed | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: |
| **macOS Terminal.app** | **Empirically Validated** | `unicode_borders` | Required | `true` | `true` | ✅ PASS |
| | | `color_profile` | Required | `Truecolor` | `Truecolor` | ✅ PASS |
| | | `no_color_degradation` | Required | `true` | `true` | ✅ PASS |
| | | `alt_w_key_routing` | Required | `true` | `true` | ✅ PASS |
| | | `osc8_hyperlinks` | Optional | `false` | `false` | ✅ PASS |
| | | `clipboard_provider` | Optional | `pbcopy (macOS)` | `pbcopy (macOS)` | ✅ PASS |
| **iTerm2** | **Simulated** | `unicode_borders` | Required | `true` | `true` | ✅ PASS |
| | | `color_profile` | Required | `Truecolor` | `Truecolor` | ✅ PASS |
| | | `no_color_degradation` | Required | `true` | `true` | ✅ PASS |
| | | `alt_w_key_routing` | Required | `true` | `true` | ✅ PASS |
| | | `osc8_hyperlinks` | Optional | `true` | `true` | ✅ PASS |
| | | `clipboard_provider` | Optional | `pbcopy (macOS)` | `pbcopy (macOS)` | ✅ PASS |
| **WezTerm** | **Simulated** | `unicode_borders` | Required | `true` | `true` | ✅ PASS |
| | | `color_profile` | Required | `Truecolor` | `Truecolor` | ✅ PASS |
| | | `no_color_degradation` | Required | `true` | `true` | ✅ PASS |
| | | `alt_w_key_routing` | Required | `true` | `true` | ✅ PASS |
| | | `osc8_hyperlinks` | Optional | `true` | `true` | ✅ PASS |
| | | `clipboard_provider` | Optional | `Platform Standard` | `Platform Standard` | ✅ PASS |
| **Ghostty** | **Simulated** | `unicode_borders` | Required | `true` | `true` | ✅ PASS |
| | | `color_profile` | Required | `Truecolor` | `Truecolor` | ✅ PASS |
| | | `no_color_degradation` | Required | `true` | `true` | ✅ PASS |
| | | `alt_w_key_routing` | Required | `true` | `true` | ✅ PASS |
| | | `osc8_hyperlinks` | Optional | `true` | `true` | ✅ PASS |
| | | `clipboard_provider` | Optional | `Platform Standard` | `Platform Standard` | ✅ PASS |
| **Alacritty** | **Simulated** | `unicode_borders` | Required | `true` | `true` | ✅ PASS |
| | | `color_profile` | Required | `Truecolor` | `Truecolor` | ✅ PASS |
| | | `no_color_degradation` | Required | `true` | `true` | ✅ PASS |
| | | `alt_w_key_routing` | Required | `true` | `true` | ✅ PASS |
| | | `osc8_hyperlinks` | Optional | `false` | `false` | ✅ PASS |
| | | `clipboard_provider` | Optional | `Platform Standard` | `Platform Standard` | ✅ PASS |
| **NO_COLOR / Plain VT** | **Simulated** | `unicode_borders` | Required | `false` (ASCII) | `false` (ASCII) | ✅ PASS |
| | | `color_profile` | Required | `NO_COLOR` | `NO_COLOR` | ✅ PASS |
| | | `no_color_degradation` | Required | `true` | `true` | ✅ PASS |
| | | `alt_w_key_routing` | Required | `true` | `true` | ✅ PASS |
| | | `osc8_hyperlinks` | Optional | `false` | `false` | ✅ PASS |
| | | `clipboard_provider` | Optional | `In-Memory Fallback` | `In-Memory Fallback` | ✅ PASS |

---

## 3. Viewport Width Invariant Test Results

Automated integration tests in `terminal_matrix_tests.rs` verify layout stability across varying terminal dimensions:

| Viewport Width | Tested Behavior & Invariants | Automated Integration Test | Status |
| :---: | :--- | :--- | :---: |
| **40 cols** | Sidebar auto-collapses into compact mode; layout renders without panic or line wrap overflow. | `test_matrix_narrow_viewport_sidebar_collapse` | ✅ PASS |
| **60 cols** | Compact panel layout dynamically adjusts flex width without panel overlap. | `test_matrix_viewport_width_invariants_no_panic` | ✅ PASS |
| **80 cols** | Standard terminal width; dual-pane sidebar and main panel display with full border margins. | `test_matrix_viewport_width_invariants_no_panic` | ✅ PASS |
| **120 cols** | Widescreen viewport; extra flex space allocated to prompt and response panels cleanly. | `test_matrix_viewport_width_invariants_no_panic` | ✅ PASS |

### Additional Integration Invariants
- **Keyboard Modifier Routing**: Alt+W shortcut routing verified across mock terminals via `test_matrix_keyboard_modifier_routing` (Status: ✅ PASS).
- **NO_COLOR ASCII Fallbacks**: Verified seamless conversion of rounded box borders (`BorderType::Rounded`) to ASCII characters (`+`, `-`, `|`) under plain terminal environments.

---

## 4. Verification Summary

```bash
# Executed Verification Pipeline:
1. ./scripts/inspect_terminal_capabilities.py target/terminal_capabilities.json
2. cargo test --test terminal_matrix_tests
3. ./scripts/validate_terminal_matrix.py
```

All 3 verification steps completed with return code `0`. All 36 capability test assertions across 6 terminal profiles passed successfully.
