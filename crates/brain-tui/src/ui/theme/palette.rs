//! Physical color definitions.

use ratatui::style::Color;

/// Raw color mappings configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Color for primary highlights.
    pub primary: Color,
    /// Color for secondary elements.
    pub secondary: Color,
    /// Color for accents.
    pub accent: Color,
    /// Color for muted/passive areas.
    pub muted: Color,
    /// Color for success states.
    pub success: Color,
    /// Color for warning states.
    pub warning: Color,
    /// Color for danger/error states.
    pub danger: Color,
    /// Color for info states.
    pub info: Color,
    /// Main body/primary text color.
    pub text_primary: Color,
    /// Secondary/accessory text color.
    pub text_secondary: Color,
    /// Main header title color.
    pub header_primary: Color,
    /// Selection highlight foreground color.
    pub selection_fg: Color,
    /// Selection highlight background color.
    pub selection_bg: Color,
    /// Color for inline code spans.
    pub code_inline: Color,
    /// Color for fenced code blocks.
    pub code_block: Color,
    /// Color for hyperlinks.
    pub link: Color,
    /// Color for active thinking spinner/text.
    pub thinking: Color,
    /// Color for active text stream chunk animations.
    pub streaming: Color,
    /// Color for user prompts/labels.
    pub user: Color,
    /// Color for assistant responses.
    pub assistant: Color,
    /// Color for tool execution messages.
    pub tool: Color,
    /// Color for system logs.
    pub system: Color,
    /// Main terminal background color.
    pub background: Color,
    /// Surface panel background color.
    pub surface: Color,
    /// Subtle divider/border color.
    pub border_subtle: Color,
    /// Command palette suggestion / category color (#AFB9F9).
    pub suggestion: Color,
}

impl Palette {
    /// Returns the default dark theme color palette.
    pub fn dark() -> Self {
        Self {
            primary: Color::Rgb(215, 119, 87),
            secondary: Color::Rgb(128, 90, 213),
            accent: Color::Rgb(215, 119, 87),
            muted: Color::Rgb(153, 153, 153),
            success: Color::Rgb(78, 186, 101),
            warning: Color::Rgb(255, 193, 7),
            danger: Color::Rgb(255, 107, 128),
            info: Color::Rgb(106, 155, 204),
            text_primary: Color::Rgb(255, 255, 255),
            text_secondary: Color::Rgb(153, 153, 153),
            header_primary: Color::Rgb(215, 119, 87), // brand orange — same as primary
            selection_fg: Color::Rgb(255, 255, 255),
            selection_bg: Color::Rgb(38, 79, 120),
            code_inline: Color::Rgb(255, 193, 7),
            code_block: Color::Rgb(153, 153, 153),
            link: Color::Rgb(106, 155, 204),
            thinking: Color::Rgb(106, 155, 204),
            streaming: Color::Rgb(215, 119, 87),
            user: Color::Rgb(106, 155, 204),
            assistant: Color::Rgb(215, 119, 87),
            tool: Color::Rgb(253, 93, 177),
            system: Color::Rgb(153, 153, 153),
            background: Color::Reset,
            surface: Color::Reset,
            border_subtle: Color::Rgb(80, 80, 80),
            suggestion: Color::Rgb(177, 185, 249),
        }
    }

    /// Returns the light theme color palette for light-background terminals.
    /// Returns the soft Paper light theme color palette.
    pub fn light() -> Self {
        Self {
            primary: Color::Rgb(192, 86, 33),          // Soft Orange
            secondary: Color::Rgb(107, 70, 193),       // Soft Purple
            accent: Color::Rgb(43, 108, 176),          // Soft Blue
            muted: Color::Rgb(102, 102, 102),          // Border & muted tone (WCAG AA compliant)
            success: Color::Rgb(20, 100, 40),          // Green (WCAG AA compliant)
            warning: Color::Rgb(140, 85, 10),          // Amber (WCAG AA compliant)
            danger: Color::Rgb(197, 48, 48),           // Red
            info: Color::Rgb(43, 108, 176),            // Blue
            text_primary: Color::Rgb(32, 33, 36),      // Dark Charcoal
            text_secondary: Color::Rgb(102, 102, 102), // Secondary Gray
            header_primary: Color::Rgb(192, 86, 33),   // brand orange — same as primary
            selection_fg: Color::White,
            selection_bg: Color::Rgb(43, 108, 176),
            code_inline: Color::Rgb(183, 121, 31),
            code_block: Color::Rgb(102, 102, 102),
            link: Color::Rgb(43, 108, 176),
            thinking: Color::Rgb(43, 108, 176),
            streaming: Color::Rgb(192, 86, 33),
            user: Color::Rgb(43, 108, 176),
            assistant: Color::Rgb(192, 86, 33),
            tool: Color::Rgb(107, 70, 193),
            system: Color::Rgb(102, 102, 102),
            background: Color::Reset, // Terminal's background color
            surface: Color::Reset,    // Terminal's background color
            border_subtle: Color::Rgb(192, 86, 33),
            suggestion: Color::Rgb(107, 70, 193),
        }
    }

    /// Returns the adaptive terminal theme palette.
    /// Invariant: Base fg/bg use `Color::Reset`, while semantic accents use explicit ANSI colors.
    pub fn terminal() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Magenta,
            accent: Color::Blue,
            muted: Color::Gray,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            info: Color::Cyan,
            text_primary: Color::Reset,
            text_secondary: Color::Reset,
            header_primary: Color::Reset,
            selection_fg: Color::Reset,
            selection_bg: Color::DarkGray,
            code_inline: Color::Yellow,
            code_block: Color::Gray,
            link: Color::Blue,
            thinking: Color::Cyan,
            streaming: Color::Magenta,
            user: Color::Cyan,
            assistant: Color::Blue,
            tool: Color::Magenta,
            system: Color::Gray,
            background: Color::Reset,
            surface: Color::Reset,
            border_subtle: Color::Reset,
            suggestion: Color::Cyan,
        }
    }

    /// Returns the high contrast accessibility palette.
    pub fn high_contrast() -> Self {
        Self {
            primary: Color::White,
            secondary: Color::White,
            accent: Color::White,
            muted: Color::Gray,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            info: Color::Cyan,
            text_primary: Color::White,
            text_secondary: Color::Gray,
            header_primary: Color::White,
            selection_fg: Color::Black,
            selection_bg: Color::White,
            code_inline: Color::Yellow,
            code_block: Color::Gray,
            link: Color::White,
            thinking: Color::White,
            streaming: Color::White,
            user: Color::White,
            assistant: Color::White,
            tool: Color::White,
            system: Color::Gray,
            background: Color::Reset,
            surface: Color::Reset,
            border_subtle: Color::White,
            suggestion: Color::White,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relative_luminance(c: Color) -> f64 {
        if let Color::Rgb(r, g, b) = c {
            let channel = |v: u8| {
                let s = v as f64 / 255.0;
                if s <= 0.03928 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
        } else {
            1.0
        }
    }

    fn contrast_ratio(c1: Color, c2: Color) -> f64 {
        let l1 = relative_luminance(c1);
        let l2 = relative_luminance(c2);
        let (max, min) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        (max + 0.05) / (min + 0.05)
    }

    #[test]
    fn test_light_palette_wcag_aa_contrast() {
        let light = Palette::light();
        let bg = light.background;

        // Text primary against background (WCAG AA > 4.5:1)
        assert!(
            contrast_ratio(light.text_primary, bg) >= 4.5,
            "TextPrimary contrast against background must be >= 4.5"
        );

        // Text secondary against background
        assert!(
            contrast_ratio(light.text_secondary, bg) >= 4.5,
            "TextSecondary contrast against background must be >= 4.5"
        );

        // Muted text against background
        assert!(
            contrast_ratio(light.muted, bg) >= 4.5,
            "Muted text contrast against background must be >= 4.5"
        );

        // Success state against background
        assert!(
            contrast_ratio(light.success, bg) >= 4.5,
            "Success color contrast against background must be >= 4.5"
        );

        // Warning state against background
        assert!(
            contrast_ratio(light.warning, bg) >= 4.5,
            "Warning color contrast against background must be >= 4.5"
        );

        // Danger state against background
        assert!(
            contrast_ratio(light.danger, bg) >= 4.5,
            "Danger color contrast against background must be >= 4.5"
        );

        // Info state against background
        assert!(
            contrast_ratio(light.info, bg) >= 4.5,
            "Info color contrast against background must be >= 4.5"
        );

        // Selection FG vs Selection BG
        assert!(
            contrast_ratio(light.selection_fg, light.selection_bg) >= 4.5,
            "Selection FG contrast against Selection BG must be >= 4.5"
        );
    }
}
