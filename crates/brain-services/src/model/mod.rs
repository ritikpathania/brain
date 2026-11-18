//! Model gateway, native ChatAgent adapter, and mock model providers.

pub mod gateway;
pub mod mock;

pub use gateway::{ChatAgentModelProvider, DefaultModelGateway};
pub use mock::{DeterministicMockProvider, ScriptedResponse};
