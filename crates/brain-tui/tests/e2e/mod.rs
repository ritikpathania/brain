//! End-to-end behavioral test suite for the brain TUI.
//!
//! ## Architectural Governance (ADR 0001)
//!
//! All E2E tests follow a strict user-journey pattern:
//! - **Arrange**: Seed deterministic sessions, knowledge candidates, or network state.
//! - **Act**: Execute user interaction sequences via keyboard input (`/`, arrows, `Enter`, `Esc`, `Tab`).
//! - **Assert**: Validate strictly observable outputs (rendered strings, selection focus, status banners, navigation targets).
//!
//! Tests verify behavioral capabilities across layers without asserting internal method calls or structure.

pub mod command_palette;
pub mod failure_modes;
pub mod navigation;
pub mod search_flow;
pub mod themes;
