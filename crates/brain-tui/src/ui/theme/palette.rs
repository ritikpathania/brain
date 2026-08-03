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
            info: Color::Rgb(106, 155, 204),
            text_primary: Color::Rgb(240, 240, 240),
            text_secondary: Color::Rgb(200, 200, 200),
            header_primary: Color::Rgb(255, 255, 255),
            selection_fg: Color::Black,
            selection_bg: Color::Rgb(173, 216, 230),
            code_inline: Color::Rgb(255, 193, 7),
            code_block: Color::Rgb(153, 153, 153),
            link: Color::Rgb(106, 155, 204),
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

    /// Returns the light theme color palette for light-background terminals.
    pub fn light() -> Self {
        Self {
            primary: Color::Rgb(180, 80, 50),
            secondary: Color::Rgb(90, 50, 160),
            accent: Color::Rgb(120, 60, 200),
            muted: Color::Rgb(100, 100, 100),
            success: Color::Rgb(30, 130, 60),
            warning: Color::Rgb(180, 120, 0),
            danger: Color::Rgb(200, 40, 60),
            info: Color::Rgb(30, 100, 180),
            text_primary: Color::Rgb(20, 20, 20),
            text_secondary: Color::Rgb(60, 60, 60),
            header_primary: Color::Rgb(10, 10, 10),
            selection_fg: Color::White,
            selection_bg: Color::Rgb(50, 100, 180),
            code_inline: Color::Rgb(160, 90, 0),
            code_block: Color::Rgb(80, 80, 80),
            link: Color::Rgb(20, 80, 160),
            thinking: Color::Rgb(30, 100, 180),
            streaming: Color::Rgb(180, 80, 50),
            user: Color::Rgb(30, 100, 180),
            assistant: Color::Rgb(180, 80, 50),
            tool: Color::Rgb(190, 40, 130),
            system: Color::Rgb(100, 100, 100),
            background: Color::Rgb(245, 245, 245),
            surface: Color::Rgb(230, 230, 230),
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
            background: Color::Black,
            surface: Color::Black,
        }
    }
}
