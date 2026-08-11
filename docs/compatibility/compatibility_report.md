# Cross-Terminal Compatibility & Capability Matrix Report

**Schema Version:** `1.0`  
**Recorded Date:** `2026-08-07`  

---

## 1. Terminal Compatibility Matrix (Three-Way Truth Model)

| Terminal Emulator Profile | Validation Status | Unicode Borders (Req) | Color Profile (Req) | `NO_COLOR` Fallback (Req) | `Alt+W` Modifier (Req) | OSC 8 Links (Opt) | Clipboard Provider (Opt) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **macOS Terminal.app** | ✅ **Empirically Validated** | PASS (`detected`: True) | PASS (`detected`: Truecolor) | PASS (`detected`: True) | PASS (`detected`: True) | PASS (`detected`: False) | PASS (`detected`: pbcopy) |
| **iTerm2** | 🔬 **Simulated** | PASS (`simulated`: True) | PASS (`simulated`: Truecolor) | PASS (`simulated`: True) | PASS (`simulated`: True) | PASS (`simulated`: True) | PASS (`simulated`: pbcopy) |
| **WezTerm** | 🔬 **Simulated** | PASS (`simulated`: True) | PASS (`simulated`: Truecolor) | PASS (`simulated`: True) | PASS (`simulated`: True) | PASS (`simulated`: True) | PASS (`simulated`: Platform Std) |
| **Ghostty** | 🔬 **Simulated** | PASS (`simulated`: True) | PASS (`simulated`: Truecolor) | PASS (`simulated`: True) | PASS (`simulated`: True) | PASS (`simulated`: True) | PASS (`simulated`: Platform Std) |
| **Alacritty** | 🔬 **Simulated** | PASS (`simulated`: True) | PASS (`simulated`: Truecolor) | PASS (`simulated`: True) | PASS (`simulated`: True) | PASS (`simulated`: False) | PASS (`simulated`: Platform Std) |
| **NO_COLOR / Plain VT** | 🔬 **Simulated** | PASS (`simulated`: False ASCII) | PASS (`simulated`: NO_COLOR) | PASS (`simulated`: True) | PASS (`simulated`: True) | PASS (`simulated`: False) | PASS (`simulated`: In-Memory) |

---

## 2. Capability Level Enforcement

- **`Required` Capabilities**: `unicode_borders`, `color_profile`, `no_color_degradation`, `alt_w_key_routing`.
  - *Enforcement*: Gates fail CI if any required capability falls below specification.
- **`Optional` Capabilities**: `osc8_hyperlinks`, `clipboard_provider`.
  - *Enforcement*: Informational tracking; degraded support falls back safely without failing build/test execution.

---

## 3. Viewport Render Invariants Across Column Widths

Verified via `cargo test --test terminal_matrix_tests`:
- **40 cols**: Ultra-narrow render draw pass; zero panic; sidebar auto-collapsed.
- **60 cols**: Narrow render draw pass; zero panic; sidebar auto-collapsed.
- **80 cols**: Standard render draw pass; full dual-pane sidebar.
- **120 cols**: Wide render draw pass; full dual-pane sidebar.
