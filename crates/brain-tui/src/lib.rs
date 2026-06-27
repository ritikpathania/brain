#![deny(missing_docs)]

//! Native Rust terminal interface client implementing the presentation layer.

/// Abstract execution client and event stream structures.
pub mod client;

/// Input event multiplexing and ticks handler.
pub mod event;
