//! Server business logic services decoupling protocol handlers from domain execution.

pub mod search;
pub use search::SearchService;
