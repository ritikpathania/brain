//! Encapsulated presentation view models mapping domain entities to renderable UI states.
//!
//! ## Architectural Invariant
//!
//! **All ViewModels in this module are immutable presentation projections.**
//!
//! ViewModels are responsible for:
//! - Formatting display strings
//! - Resolving `None` placeholder values at the presentation boundary
//! - Computing derived display values (badges, highlight ranges)
//! - Carrying presentation metadata (confidence tier, source kind)
//!
//! ViewModels must NOT contain:
//! - Retrieval or ranking logic (belongs in `search::ranking`)
//! - Transport state or network error handling (belongs in `client`)
//! - Mutable UI state or selection indices (belongs in widget state)
//! - Grouping, filtering, or sorting algorithms (belongs in dedicated engines)
//!
//! This constraint prevents ViewModels from becoming "smart objects" that
//! accumulate retrieval semantics over time. The pipeline is strictly:
//!
//! ```text
//! Domain entity / SearchResult  (retrieval boundary)
//!         ↓
//! ViewModel::from_*             (projection boundary — this module)
//!         ↓
//! Renderer / Widget             (display boundary)
//! ```

pub mod inspection_session;
pub mod inspector;
pub mod memory_results;
pub mod memory_search_results;
pub mod reasoning_plan;
pub mod search_results;

pub use inspection_session::{InspectionLocation, InspectionSession};
pub use inspector::{EntitySection, EntitySectionId, InspectorViewModel, RelationshipViewModel};
pub use memory_results::{MemoryItemViewModel, MemoryResultsViewModel};
pub use memory_search_results::{
    DetailAvailability, MemoryGroupingEngine, MemoryResultGroup, MemoryResultViewModel,
};
pub use reasoning_plan::{ReasoningPlanViewModel, ReasoningStepViewModel};
pub use search_results::{SearchResultViewModel, SearchResultsViewModel};
