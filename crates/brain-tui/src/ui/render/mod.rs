//! Stateless rendering helpers and drawing contexts.

pub mod border;
pub mod context;
pub mod icon;
/// Border and link presentation resolvers.
pub mod resolver;
pub mod text;

pub use border::BorderRenderer;
pub use context::{
    CapabilityPolicy, CapabilityResolver, ColorSupport, EffectiveCapabilities, MotionPreference,
    NerdFontsSupport, RenderCapabilities, RenderContext, UnicodeSupport,
};
pub use icon::IconSet;
pub use resolver::{BorderGlyphs, BorderResolver, LinkRenderer, TerminalSpan};
pub use text::TextRenderer;
