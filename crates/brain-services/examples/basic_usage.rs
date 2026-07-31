//! End-to-end example demonstrating how to initialize and interact with `BrainRuntime` directly in Rust.
//!
//! Run with:
//! ```bash
//! cargo run --example basic_usage -p brain-services
//! ```

use brain_services::BrainRuntime;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Brain Relational Memory Engine: Basic Usage Example ===\n");

    // 1. Initialize a temporary database file
    let temp_db = NamedTempFile::new()?;
    let db_path_str = temp_db.path().to_str().expect("valid temp path");
    println!(
        "[1/3] Initializing BrainRuntime at temporary SQLite database: {}",
        db_path_str
    );

    // 2. Instantiate BrainRuntime composition root
    let runtime = BrainRuntime::new(db_path_str)?;
    println!("[2/3] BrainRuntime composition root initialized successfully.");

    // 3. Capture an atomic point-in-time runtime diagnostics snapshot
    let snapshot = runtime.diagnostics_snapshot();
    println!("[3/3] Runtime Diagnostics Snapshot:");
    println!("  - Snapshot Sequence ID: {}", snapshot.snapshot_sequence);
    println!("  - Runtime Health: {:?}", snapshot.health);
    println!(
        "  - Projection Lag Count: {}",
        snapshot.projection_lags.len()
    );

    // Gracefully shutdown runtime
    let shutdown_summary = runtime.shutdown()?;
    println!(
        "\n  - Shutdown completed in {:?}",
        shutdown_summary.duration
    );
    println!("\n=== Brain Runtime Basic Usage Example Completed Successfully! ===");

    Ok(())
}
