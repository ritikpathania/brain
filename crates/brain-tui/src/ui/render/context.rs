//! Unified RenderContext configuration and terminal capability discovery.

use crate::ui::render::icon::IconSet;
use crate::ui::theme::ActiveTheme;

/// Supported levels of Unicode symbol rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnicodeSupport {
    /// Full UTF-8 support including icons and emoji.
    Full,
    /// Fallback to ASCII character shapes only.
    AsciiOnly,
}

/// Supported levels of color rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorSupport {
    /// Truecolor (RGB) support.
    TrueColor,
    /// 256-color (ANSI) support.
    Ansi256,
    /// Basic 16-color (ANSI) support.
    Ansi16,
}

/// Supported levels of Nerd Font icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NerdFontsSupport {
    /// Full Nerd Fonts symbols available.
    Full,
    /// No Nerd Font icons.
    None,
}

/// Motion preference representing visual animation constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionPreference {
    /// Full animations (spinners, tickers).
    Full,
    /// Disabled or static indicators only.
    Reduced,
}

/// Set of terminal capabilities observed at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderCapabilities {
    /// Observed level of Unicode symbol support.
    pub unicode: UnicodeSupport,
    /// Observed color rendering depth.
    pub colors: ColorSupport,
    /// Observed Nerd Font symbols support.
    pub nerd_fonts: NerdFontsSupport,
    /// Whether mouse events can be captured.
    pub mouse: bool,
    /// Whether the terminal supports OSC-8 hyperlinks.
    pub osc8: bool,
    /// Observed motion animation constraints.
    pub motion: MotionPreference,
}

impl RenderCapabilities {
    /// Observes the terminal environment to detect capability support levels.
    pub fn detect() -> Self {
        let has_utf8 = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .or_else(|_| std::env::var("LC_CTYPE"))
            .map(|s| s.to_uppercase().contains("UTF-8"))
            .unwrap_or(false);

        let unicode = if has_utf8 {
            UnicodeSupport::Full
        } else {
            UnicodeSupport::AsciiOnly
        };

        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        let colors = if colorterm == "truecolor" || colorterm == "24bit" {
            ColorSupport::TrueColor
        } else {
            ColorSupport::Ansi256
        };

        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        let term = std::env::var("TERM").unwrap_or_default();

        let nerd_fonts = if std::env::var("NERD_FONTS")
            .map(|s| s == "1" || s.to_uppercase() == "TRUE")
            .unwrap_or(term_program == "WezTerm" || term_program == "ghostty")
        {
            NerdFontsSupport::Full
        } else {
            NerdFontsSupport::None
        };

        let osc8 = term_program == "iTerm.app"
            || term_program == "WezTerm"
            || term_program == "ghostty"
            || term_program == "vscode"
            || term.contains("kitty");

        let motion = if std::env::var("REDUCED_MOTION")
            .map(|s| s == "1" || s.to_uppercase() == "TRUE")
            .unwrap_or(false)
        {
            MotionPreference::Reduced
        } else {
            MotionPreference::Full
        };

        Self {
            unicode,
            colors,
            nerd_fonts,
            mouse: true,
            osc8,
            motion,
        }
    }
}

/// Set of user-desired capability policy overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CapabilityPolicy {
    /// Force rendering using ASCII boundaries only.
    pub force_ascii: bool,
    /// Explicit override for color depth support.
    pub force_colors: Option<ColorSupport>,
    /// Disable mouse events capture.
    pub disable_mouse: bool,
    /// Disable tickers and cursor animations.
    pub disable_motion: bool,
}

/// Immutable, resolved capability configuration utilized by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectiveCapabilities {
    /// Resolved level of Unicode symbol support.
    pub unicode: UnicodeSupport,
    /// Resolved color rendering depth.
    pub colors: ColorSupport,
    /// Resolved Nerd Font symbols support.
    pub nerd_fonts: NerdFontsSupport,
    /// Resolved mouse capability flag.
    pub mouse: bool,
    /// Resolved OSC-8 hyperlinks flag.
    pub osc8: bool,
    /// Resolved motion constraints.
    pub motion: MotionPreference,
}

/// Pure associated resolver mapping observed capabilities and preferences to effective configurations.
pub struct CapabilityResolver;

impl CapabilityResolver {
    /// Pure associated function resolving observed capabilities and policy guidelines.
    pub fn resolve(caps: &RenderCapabilities, policy: &CapabilityPolicy) -> EffectiveCapabilities {
        let unicode = if policy.force_ascii {
            UnicodeSupport::AsciiOnly
        } else {
            caps.unicode
        };

        let nerd_fonts = if policy.force_ascii {
            NerdFontsSupport::None
        } else {
            caps.nerd_fonts
        };

        let colors = if let Some(forced) = policy.force_colors {
            forced
        } else {
            caps.colors
        };

        let mouse = caps.mouse && !policy.disable_mouse;

        let motion = if policy.disable_motion {
            MotionPreference::Reduced
        } else {
            caps.motion
        };

        EffectiveCapabilities {
            unicode,
            colors,
            nerd_fonts,
            mouse,
            osc8: caps.osc8,
            motion,
        }
    }
}

/// Context passed to stateless UI primitives and rendering helpers.
pub struct RenderContext<'a, T: ActiveTheme> {
    /// Active theme instance resolving token colors.
    pub theme: &'a T,
    /// Active icon mappings.
    pub icons: &'a IconSet,
    /// Effective capabilities configuration.
    pub capabilities: EffectiveCapabilities,
    /// Monotonically increasing tick count driving animations.
    pub tick: usize,
}
