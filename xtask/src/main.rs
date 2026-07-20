use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "generate-contracts" => generate_contracts()?,
            "verify-contracts" => verify_contracts()?,
            "regenerate-retrieval-baselines" => regenerate_retrieval_baselines()?,
            "architecture-check" => architecture_check()?,
            "verify" => verify()?,
            _ => {
                eprintln!(
                    "Usage: cargo xtask [generate-contracts | verify-contracts \
                     | regenerate-retrieval-baselines | architecture-check | verify]"
                );
                std::process::exit(1);
            }
        }
    } else {
        eprintln!(
            "Usage: cargo xtask [generate-contracts | verify-contracts \
             | regenerate-retrieval-baselines | architecture-check | verify]"
        );
        std::process::exit(1);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// verify — unified Rust quality gate
// ---------------------------------------------------------------------------

/// Runs all deterministic, offline Rust checks in order.
///
/// Stops at the first failure unless the `XTASK_KEEP_GOING` environment
/// variable is set to `1`.
///
/// Python checks (ruff, ty) are intentionally excluded: they require a
/// virtual environment and are orthogonal to the Rust toolchain.  Run them
/// via `make verify` in CI or locally.
///
/// `cargo deny check advisories` is intentionally excluded: it requires
/// network access to refresh the advisory DB.  It runs as a dedicated CI job.
fn verify() -> Result<(), Box<dyn std::error::Error>> {
    let keep_going = std::env::var("XTASK_KEEP_GOING").as_deref() == Ok("1");

    struct Step {
        label: &'static str,
        cmd: &'static str,
        args: &'static [&'static str],
    }

    #[derive(Clone)]
    enum Outcome {
        Pass,
        Fail,
        Skip,
    }

    let steps = [
        Step {
            label: "fmt --check",
            cmd: "cargo",
            args: &["fmt", "--all", "--", "--check"],
        },
        Step {
            label: "clippy",
            cmd: "cargo",
            args: &["clippy", "--all-targets", "--", "-D", "warnings"],
        },
        Step {
            label: "test --all",
            cmd: "cargo",
            args: &["test", "--all"],
        },
        Step {
            label: "verify-contracts",
            cmd: "cargo",
            args: &["xtask", "verify-contracts"],
        },
        Step {
            label: "architecture-check",
            cmd: "cargo",
            args: &["xtask", "architecture-check"],
        },
    ];

    println!("\ncargo xtask verify — Rust quality gate");
    println!("{}", "─".repeat(56));

    let mut outcomes: Vec<(&'static str, Outcome)> = Vec::new();
    let mut any_failed = false;
    let mut stopped_early = false;
    let total_start = Instant::now();

    for step in &steps {
        let start = Instant::now();
        print!("  {:30} … ", step.label);
        let status = Command::new(step.cmd).args(step.args).status()?;
        let elapsed = start.elapsed();

        if status.success() {
            println!("[ PASS ] ({:.1}s)", elapsed.as_secs_f32());
            outcomes.push((step.label, Outcome::Pass));
        } else {
            println!("[ FAIL ] ({:.1}s)", elapsed.as_secs_f32());
            outcomes.push((step.label, Outcome::Fail));
            any_failed = true;
            if !keep_going {
                stopped_early = true;
                break;
            }
        }
    }

    // Steps skipped due to early exit.
    if stopped_early {
        let ran = outcomes.len();
        for step in steps.iter().skip(ran) {
            outcomes.push((step.label, Outcome::Skip));
        }
    }

    // Nextest: optional, run only if cargo-nextest is on PATH.
    let nextest_available = Command::new("cargo")
        .args(["nextest", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !stopped_early {
        if nextest_available {
            let start = Instant::now();
            print!("  {:30} … ", "nextest run");
            let status = Command::new("cargo")
                .args(["nextest", "run", "--all"])
                .status()?;
            let elapsed = start.elapsed();
            if status.success() {
                println!("[ PASS ] ({:.1}s)", elapsed.as_secs_f32());
                outcomes.push(("nextest run", Outcome::Pass));
            } else {
                println!("[ FAIL ] ({:.1}s)", elapsed.as_secs_f32());
                outcomes.push(("nextest run", Outcome::Fail));
                any_failed = true;
            }
        } else {
            println!(
                "  {:30} … [ SKIP ] (cargo-nextest not installed)",
                "nextest run"
            );
            outcomes.push(("nextest run", Outcome::Skip));
        }
    } else {
        outcomes.push(("nextest run", Outcome::Skip));
    }

    let total_elapsed = total_start.elapsed();

    // ── Summary ──────────────────────────────────────────────────────────────
    println!("\n{}", "─".repeat(56));
    println!("  Verification Summary\n");

    let mut skipped_labels: Vec<&str> = Vec::new();
    for (label, outcome) in &outcomes {
        match outcome {
            Outcome::Pass => println!("  ✓ {}", label),
            Outcome::Fail => println!("  ✗ {}", label),
            Outcome::Skip => skipped_labels.push(label),
        }
    }

    if !skipped_labels.is_empty() {
        println!("\n  Skipped:");
        for label in &skipped_labels {
            println!("    - {}", label);
        }
    }

    let result_label = if any_failed { "FAIL" } else { "PASS" };
    println!("\n  Result:  {}", result_label);
    println!("  Elapsed: {:.1}s", total_elapsed.as_secs_f32());
    println!("{}", "─".repeat(56));

    if any_failed {
        return Err("One or more quality gate steps failed.".into());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// architecture-check
// ---------------------------------------------------------------------------

/// Delegates to the `brain-arch-tests` integration-test crate.
///
/// Both `cargo xtask architecture-check` and `cargo test --all` run the same
/// tests — there is a single implementation.
fn architecture_check() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running architecture boundary tests (brain-arch-tests)…");
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "brain-arch-tests",
            "--test",
            "dependency_boundaries",
            "--",
            "--nocapture",
        ])
        .status()?;
    if !status.success() {
        return Err("Architecture boundary tests failed.".into());
    }
    println!("Architecture boundary tests: PASSED");
    Ok(())
}

// ---------------------------------------------------------------------------
// regenerate-retrieval-baselines
// ---------------------------------------------------------------------------

fn regenerate_retrieval_baselines() -> Result<(), Box<dyn std::error::Error>> {
    println!("Regenerating retrieval baselines via cargo test...");
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "brain-services",
            "--test",
            "fts_benchmark_tests",
            "--",
            "test_fts_benchmark_cold_and_warm_cache",
        ])
        .env("REGENERATE_BASELINES", "1")
        .env("DYLD_FRAMEWORK_PATH", "/Library/Developer/CommandLineTools/Library/Frameworks")
        .env("LIBRARY_PATH", "/Library/Developer/CommandLineTools/Library/Frameworks/Python3.framework/Versions/3.9/lib")
        .status()?;

    if !status.success() {
        return Err("Failed to regenerate baselines via cargo test".into());
    }

    println!("Retrieval baselines successfully regenerated!");
    Ok(())
}

// ---------------------------------------------------------------------------
// generate-contracts / verify-contracts
// ---------------------------------------------------------------------------

fn generate_contracts_to_string() -> Result<String, Box<dyn std::error::Error>> {
    // Version metadata
    let contract_version = "1.0.0";
    let generator_version = env!("CARGO_PKG_VERSION");
    let brain_version = "0.1.0";

    // Registry configuration
    let config = specta::ts::ExportConfiguration::default();

    // Explicit contract registry list
    let mut types_to_export = vec![
        (
            "Value",
            specta::ts::export::<brain_integrations::Value>(&config)?,
        ),
        (
            "Capability",
            specta::ts::export::<brain_integrations::Capability>(&config)?,
        ),
        (
            "EventIdentity",
            specta::ts::export::<brain_integrations::EventIdentity>(&config)?,
        ),
        (
            "IngestionEvent",
            specta::ts::export::<brain_integrations::IngestionEvent>(&config)?,
        ),
        (
            "IngestionEnvelope",
            specta::ts::export::<brain_integrations::IngestionEnvelope>(&config)?,
        ),
    ];

    // Sort alphabetically by type name to ensure deterministic output order
    types_to_export.sort_by_key(|(name, _)| *name);

    // Build typescript type definitions without any volatile timestamps
    let mut ts_content = String::new();
    ts_content
        .push_str("// ----------------------------------------------------------------------\n");
    ts_content.push_str("// GENERATED FILE\n");
    ts_content.push_str("//\n");
    ts_content.push_str("// Source:\n");
    ts_content.push_str("//   brain-integrations DTO registry\n");
    ts_content.push_str("//\n");
    ts_content.push_str("// Generated by:\n");
    ts_content.push_str("//   cargo xtask generate-contracts\n");
    ts_content.push_str("//\n");
    ts_content.push_str("// DO NOT EDIT\n");
    ts_content
        .push_str("// ----------------------------------------------------------------------\n");
    ts_content.push_str(&format!("// Contract Version:  {}\n", contract_version));
    ts_content.push_str(&format!("// Generator Version: {}\n", generator_version));
    ts_content.push_str(&format!("// Brain Version:     {}\n", brain_version));
    ts_content
        .push_str("// ----------------------------------------------------------------------\n\n");

    for (_name, definition) in types_to_export {
        ts_content.push_str(&definition);
        ts_content.push_str("\n\n");
    }

    Ok(ts_content)
}

fn generate_contracts() -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating contracts...");
    let ts_content = generate_contracts_to_string()?;

    // Atomic generate workflow: Write to temp first
    let temp_dir = Path::new("temp_generated");
    fs::create_dir_all(temp_dir)?;

    let temp_file_path = temp_dir.join("types.ts");
    fs::write(&temp_file_path, &ts_content)?;

    // Basic structural validation check
    if ts_content.is_empty() || !ts_content.contains("export type IngestionEnvelope") {
        return Err("Validation failed: Output TypeScript is malformed or empty".into());
    }

    // Atomic overwrite to target directories
    let output_dir = Path::new("generated/typescript");
    fs::create_dir_all(output_dir)?;
    let output_file_path = output_dir.join("types.ts");

    let sdk_dir = Path::new("sdks/typescript/src/generated");
    fs::create_dir_all(sdk_dir)?;
    let sdk_file_path = sdk_dir.join("types.ts");

    fs::copy(&temp_file_path, &output_file_path)?;
    fs::copy(&temp_file_path, &sdk_file_path)?;
    fs::remove_file(&temp_file_path)?;
    fs::remove_dir(temp_dir)?;

    println!(
        "Contracts successfully generated at: \n  - generated/typescript/types.ts\n  - sdks/typescript/src/generated/types.ts"
    );
    Ok(())
}

fn verify_contracts() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting contract verification...");

    // 1. Verify determinism
    println!("Running determinism verification...");
    let run_a = generate_contracts_to_string()?;
    let run_b = generate_contracts_to_string()?;
    if run_a != run_b {
        return Err("Determinism check failed: contract generation is non-deterministic.".into());
    }
    println!("  Determinism verification check: PASSED");

    // 2. Verify freshness
    println!("Running freshness verification...");
    let in_memory = generate_contracts_to_string()?;

    let output_file_path = Path::new("generated/typescript/types.ts");
    let sdk_file_path = Path::new("sdks/typescript/src/generated/types.ts");
    if !output_file_path.exists() || !sdk_file_path.exists() {
        return Err(
            "Freshness check failed: generated types files do not exist.\n\
             Action required:\n\
             Run:\n\
                 cargo xtask generate-contracts\n"
                .to_string()
                .into(),
        );
    }

    let on_disk_root = fs::read_to_string(output_file_path)?;
    let on_disk_sdk = fs::read_to_string(sdk_file_path)?;
    if in_memory.trim() != on_disk_root.trim() || in_memory.trim() != on_disk_sdk.trim() {
        return Err(
            "\n======================================================================\n\
             VERIFICATION ERROR: Generated artifacts differ from the committed contract.\n\n\
             Action required:\n\
             Run:\n\
                 cargo xtask generate-contracts\n\n\
             Review:\n\
                 git diff generated/ sdks/typescript/src/generated/\n\n\
             Then commit the updated generated artifacts if the change is intentional.\n\
             ======================================================================"
                .to_string()
                .into(),
        );
    }
    println!("  Freshness verification check: PASSED");

    // 3. Repository cleanliness check (convenience check via git)
    println!("Running repository cleanliness check...");
    let git_status = Command::new("git")
        .args([
            "diff",
            "--exit-code",
            "generated/",
            "sdks/typescript/src/generated/",
        ])
        .status();

    match git_status {
        Ok(status) if status.success() => {
            println!("  Repository cleanliness check: PASSED");
        }
        _ => {
            println!("  Warning: Git diff shows uncommitted changes under generated/ or sdks/typescript/src/generated/ directories.");
        }
    }

    println!("All contract verification quality gates: PASSED!");
    Ok(())
}
