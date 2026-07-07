//! Stateless rendering helpers and drawing contexts.

pub mod icon;
pub mod context;
pub mod text;
pub mod border;
/// Border and link presentation resolvers.
pub mod resolver;

pub use icon::IconSet;
pub use context::{
    RenderContext, RenderCapabilities, UnicodeSupport, ColorSupport, NerdFontsSupport,
    MotionPreference, CapabilityPolicy, EffectiveCapabilities, CapabilityResolver
};
pub use text::TextRenderer;
pub use border::BorderRenderer;
pub use resolver::{BorderGlyphs, BorderResolver, TerminalSpan, LinkRenderer};
