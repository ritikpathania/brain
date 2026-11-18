//! Kernel-enforced filesystem sandboxing and isolation drivers (SEC-03).
//!
//! Provides platform-native sandbox drivers (Apple Seatbelt on macOS, Landlock LSM on Linux)
//! and process-boundary isolation alternatives with a unified, platform-independent [`SandboxPolicy`].

pub mod driver;
pub mod landlock;
pub mod policy;
pub mod process_boundary;
pub mod seatbelt;

pub use driver::{
    EnforcementMode, MockSandboxDriver, NoopSandboxDriver, SandboxActivationReport, SandboxDriver,
};
pub use landlock::LandlockSandboxDriver;
pub use policy::{
    EnvPolicy, FsAccess, FsRule, SandboxPolicy, SandboxPolicyBuilder, SensitivePathRule,
};
pub use process_boundary::ProcessBoundaryDriver;
pub use seatbelt::SeatbeltSandboxDriver;

use std::sync::Arc;

/// Resolves the default sandbox driver for the current operating system and runtime environment.
///
/// Resolution precedence:
/// 1. `BRAIN_SANDBOX_DRIVER` environment variable override:
///    - `"seatbelt"`: forces Apple Seatbelt driver (fails closed if unavailable)
///    - `"landlock"`: forces Linux Landlock driver (fails closed if unavailable)
///    - `"process"` | `"process_boundary"`: process-level boundary isolation alternative
///    - `"noop"`: no-op baseline driver (fails closed when enforced)
///    - `"mock"`: mock sandbox driver for deterministic adversarial testing
/// 2. Platform-native kernel drivers:
///    - On macOS: checks `SeatbeltSandboxDriver::is_supported()`. If operational, returns Seatbelt.
///      If Seatbelt is restricted or unavailable (e.g. macOS SDK >= 27.0 deprecation, nested sandbox),
///      engages [`ProcessBoundaryDriver`] isolation alternative per R3.
///    - On Linux: checks `LandlockSandboxDriver::is_supported()`. If kernel ABI >= 1, returns Landlock.
///      Otherwise engages [`ProcessBoundaryDriver`] isolation alternative.
///    - Unsupported platforms: returns [`NoopSandboxDriver`].
pub fn default_sandbox_driver() -> Arc<dyn SandboxDriver> {
    if let Ok(driver_env) = std::env::var("BRAIN_SANDBOX_DRIVER") {
        match driver_env.to_lowercase().as_str() {
            "seatbelt" => return Arc::new(SeatbeltSandboxDriver::new()),
            "landlock" => return Arc::new(LandlockSandboxDriver::new()),
            "process" | "process_boundary" => return Arc::new(ProcessBoundaryDriver::new()),
            "noop" => return Arc::new(NoopSandboxDriver::new()),
            "mock" => return Arc::new(MockSandboxDriver::new()),
            _ => {}
        }
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, return native Seatbelt driver. If unsupported or restricted,
        // SeatbeltSandboxDriver fails closed under EnforcementMode::Enforced
        // without silently falling back to unconfined or advisory-only execution.
        Arc::new(SeatbeltSandboxDriver::new())
    }
    #[cfg(target_os = "linux")]
    {
        let landlock = LandlockSandboxDriver::new();
        if landlock.is_supported() {
            Arc::new(landlock)
        } else {
            // Per R4: Fail closed if the required restrictions cannot be enforced.
            // Do not silently substitute advisory filtering for unavailable kernel enforcement.
            Arc::new(NoopSandboxDriver::new())
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Arc::new(NoopSandboxDriver::new())
    }
}
