//! Event stream logical position watermark.

use serde::{Deserialize, Serialize};

/// Monotonic event stream logical position watermark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Watermark(pub u64);
