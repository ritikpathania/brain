# Theme Tokens Mappings

This reference document defines the physical color mappings (RGB, ANSI-256, ANSI-16, and Daltonized equivalents) and contrast compliance checks for each semantic design token in the Brain UI.

---

## 1. Color Value Mappings

The table below catalogs the exact color codes to be loaded by the active TUI client theme resolvers:

| Token Name | Dark Theme (RGB) | Light Theme (RGB) | Daltonized (RGB) | ANSI-256 Code | ANSI-16 Fallback | Contrast Ratio (Dark) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Primary** | `rgb(215,119,87)` | `rgb(195,99,67)` | `rgb(220,160,30)` | `208` | `LightRed` | 4.8:1 |
| **Secondary** | `rgb(128,90,213)` | `rgb(98,60,183)` | `rgb(40,110,190)` | `99` | `LightMagenta`| 5.1:1 |
| **Accent** | `rgb(175,135,255)`| `rgb(135,95,215)` | `rgb(120,150,250)`| `141` | `Magenta` | 4.9:1 |
| **Muted** | `rgb(153,153,153)`| `rgb(110,110,110)`| `rgb(153,153,153)`| `245` | `DarkGray` | 3.2:1 |
| **Success** | `rgb(78,186,101)` | `rgb(48,156,71)`  | `rgb(50,150,220)` | `71` | `LightGreen` | 5.3:1 |
| **Warning** | `rgb(255,193,7)`   | `rgb(215,153,0)`  | `rgb(240,240,0)`  | `220` | `LightYellow` | 6.2:1 |
| **Danger** | `rgb(255,107,128)`| `rgb(215,67,88)`  | `rgb(215,80,0)`   | `203` | `LightRed` | 4.6:1 |
| **Thinking** | `rgb(106,155,204)`| `rgb(76,125,174)` | `rgb(70,140,200)` | `75` | `LightCyan` | 5.5:1 |
| **Streaming** | `rgb(215,119,87)` | `rgb(195,99,67)`  | `rgb(220,160,30)` | `208` | `LightRed` | 4.8:1 |
| **User** | `rgb(106,155,204)`| `rgb(76,125,174)` | `rgb(70,140,200)` | `75` | `LightCyan` | 5.5:1 |
| **Assistant** | `rgb(215,119,87)` | `rgb(195,99,67)`  | `rgb(220,160,30)` | `208` | `LightRed` | 4.8:1 |
| **Tool** | `rgb(253,93,177)`  | `rgb(213,53,137)` | `rgb(200,90,180)` | `205` | `LightMagenta`| 4.7:1 |
| **System** | `rgb(153,153,153)`| `rgb(110,110,110)`| `rgb(153,153,153)`| `245` | `DarkGray` | 3.2:1 |

---

## 2. Daltonized Theme Specification (Colorblind Accommodations)
* **Goal**: Maximize red-green differentiation.
* **Rules**:
  1. The **Primary** brand color is shifted from standard warm orange to yellow-amber `rgb(220,160,30)` to avoid confusion with red errors.
  2. The **Success** token shifts from standard green to blue-sky `rgb(50,150,220)`, ensuring that users with protanopia or deuteranopia can easily tell successful completions apart from errors (which shift from pink-red to high-contrast orange-rust `rgb(215,80,0)`).
  3. Muted elements remain high-contrast neutral gray.

---

## 3. Shimmer Effects (Visual Progress Animation)
Certain progress states apply a visual cyclic "shimmer" (brightness oscillations). The theme mappings for shimmer ranges are:
* **Primary Shimmer**: Oscillates between `rgb(215,119,87)` and `rgb(245,149,117)` over a 1200ms duration.
* **Success Shimmer**: Oscillates between `rgb(78,186,101)` and `rgb(108,216,131)`.
* **Warning Shimmer**: Oscillates between `rgb(255,193,7)` and `rgb(255,223,67)`.
* **Thinking Shimmer**: Oscillates between `rgb(106,155,204)` and `rgb(136,185,234)`.
