//! Screen components rendering full-viewport layouts.

/// Knowledge Graph Explorer screen.
pub mod graph_explorer;

pub use graph_explorer::{GraphExplorerScreen, GraphExplorerScreenState};

/// Reflection & Memory Stewardship screen.
pub mod reflection;
pub use reflection::{ReflectionScreen, ReflectionScreenState};

/// Knowledge Evolution screen.
pub mod evolution;
pub use evolution::{EvolutionScreen, EvolutionScreenState};
