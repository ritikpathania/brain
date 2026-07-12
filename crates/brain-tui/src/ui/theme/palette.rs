//! Physical color definitions.

use ratatui::style::Color;

/// Raw color mappings configuration.
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
    /// Sub-panel surface color.
    pub surface: Color,
}

impl Palette {
    /// Returns the default dark theme color palette.
    pub fn dark() -> Self {
        Self {
            primary: Color::Rgb(215, 119, 87),
            secondary: Color::Rgb(128, 90, 213),
            accent: Color::Rgb(175, 135, 255),
            muted: Color::Rgb(153, 153, 153),
            success: Color::Rgb(78, 186, 101),
            warning: Color::Rgb(255, 193, 7),
            danger: Color::Rgb(255, 107, 128),
            thinking: Color::Rgb(106, 155, 204),
            streaming: Color::Rgb(215, 119, 87),
            user: Color::Rgb(106, 155, 204),
            assistant: Color::Rgb(215, 119, 87),
            tool: Color::Rgb(253, 93, 177),
            system: Color::Rgb(153, 153, 153),
            background: Color::Black,
            surface: Color::Rgb(55, 55, 55),
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
            thinking: Color::White,
            streaming: Color::White,
            user: Color::White,
            assistant: Color::White,
            tool: Color::White,
            system: Color::Gray,
            background: Color::Black,
            surface: Color::Black,
        }
    }
}

