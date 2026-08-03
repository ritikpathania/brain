//! Encapsulated presentation view models mapping domain entities to renderable UI states.

pub mod inspection_session;
pub mod inspector;
pub mod memory_results;
pub mod reasoning_plan;
pub mod search_results;

pub use inspection_session::{InspectionLocation, InspectionSession};
pub use inspector::{EntitySection, EntitySectionId, InspectorViewModel, RelationshipViewModel};
pub use memory_results::{MemoryItemViewModel, MemoryResultsViewModel};
pub use reasoning_plan::{ReasoningPlanViewModel, ReasoningStepViewModel};
pub use search_results::{SearchResultViewModel, SearchResultsViewModel};
