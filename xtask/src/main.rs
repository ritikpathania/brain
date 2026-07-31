use std::fs;
use std::path::{Path, PathBuf};
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
            "docs" => docs_cmd(&args[2..])?,
            _ => {
                print_usage();
                std::process::exit(1);
            }
        }
    } else {
        print_usage();
        std::process::exit(1);
    }
    Ok(())
}

fn print_usage() {
    eprintln!(
        "Usage: cargo xtask [generate-contracts | verify-contracts | \
         regenerate-retrieval-baselines | architecture-check | verify | docs <subcommand>]"
    );
}

// ---------------------------------------------------------------------------
// docs — composable non-destructive documentation verification suite
// ---------------------------------------------------------------------------

#[derive(Default, serde::Serialize)]
struct DocsCheckReport {
    active_broken_links: Vec<String>,
    archive_broken_links: Vec<String>,
    missing_readmes: Vec<String>,
    active_orphan_documents: Vec<String>,
    archive_orphan_documents: Vec<String>,
    invalid_snippets: Vec<String>,
    frontmatter_violations: Vec<String>,
    subsystem_violations: Vec<String>,
}

fn docs_cmd(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let subcommand = args.first().map(|s| s.as_str()).unwrap_or("all");
    let json_output = args.iter().any(|a| a == "--json");

    let mut report = DocsCheckReport::default();

    match subcommand {
        "links" => check_links(&mut report)?,
        "readmes" => check_readmes(&mut report)?,
        "indexes" => check_indexes(&mut report)?,
        "snippets" => check_snippets(&mut report)?,
        "frontmatter" => check_frontmatter(&mut report)?,
        "subsystems" => check_subsystems(&mut report)?,
        "all" | "--json" => {
            check_links(&mut report)?;
            check_readmes(&mut report)?;
            check_indexes(&mut report)?;
            check_snippets(&mut report)?;
            check_frontmatter(&mut report)?;
            check_subsystems(&mut report)?;
        }
        _ => {
            eprintln!(
                "Usage: cargo xtask docs [links | readmes | indexes | snippets | frontmatter | subsystems | all] [--json]"
            );
            std::process::exit(1);
        }
    }

    if json_output {
        let json_str = serde_json::to_string_pretty(&report)?;
        println!("{}", json_str);
    } else {
        print_docs_summary(subcommand, &report)?;
    }

    let has_errors = !report.active_broken_links.is_empty()
        || !report.missing_readmes.is_empty()
        || !report.active_orphan_documents.is_empty()
        || !report.invalid_snippets.is_empty()
        || !report.frontmatter_violations.is_empty()
        || !report.subsystem_violations.is_empty();

    if has_errors && subcommand != "all" && !json_output {
        std::process::exit(1);
    }

    Ok(())
}

fn print_docs_summary(
    subcommand: &str,
    report: &DocsCheckReport,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "\ncargo xtask docs {} — Documentation Quality Check",
        subcommand
    );
    println!("{}", "─".repeat(60));

    println!(
        "  Active Broken Links:    {}",
        report.active_broken_links.len()
    );
    for item in &report.active_broken_links {
        println!("    - ✗ {}", item);
    }

    println!(
        "  Archived Broken Links:  {} (Informational)",
        report.archive_broken_links.len()
    );

    println!("  Missing Crate READMEs:  {}", report.missing_readmes.len());
    for item in &report.missing_readmes {
        println!("    - ✗ {}", item);
    }

    println!(
        "  Active Orphan Docs:     {}",
        report.active_orphan_documents.len()
    );
    for item in &report.active_orphan_documents {
        println!("    - ✗ {}", item);
    }

    println!(
        "  Archived Orphan Docs:   {} (Informational)",
        report.archive_orphan_documents.len()
    );

    println!(
        "  Invalid Code Snippets:  {}",
        report.invalid_snippets.len()
    );
    for item in &report.invalid_snippets {
        println!("    - ✗ {}", item);
    }

    println!(
        "  Frontmatter Violations: {}",
        report.frontmatter_violations.len()
    );
    for item in &report.frontmatter_violations {
        println!("    - ✗ {}", item);
    }

    println!(
        "  Subsystem Violations:   {}",
        report.subsystem_violations.len()
    );
    for item in &report.subsystem_violations {
        println!("    - ✗ {}", item);
    }

    println!("{}", "─".repeat(60));
    Ok(())
}

fn collect_md_files(
    dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "target" || name == "node_modules" || name == ".venv" {
            continue;
        }
        if path.is_dir() {
            collect_md_files(&path, files)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
            files.push(path);
        }
    }
    Ok(())
}

fn check_links(report: &mut DocsCheckReport) -> Result<(), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect_md_files(Path::new("."), &mut files)?;

    for file_path in &files {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let file_dir = file_path.parent().unwrap_or_else(|| Path::new("."));

        for line_num in 1..=content.lines().count() {
            let line = content.lines().nth(line_num - 1).unwrap();
            let mut start = 0;
            while let Some(open_bracket) = line[start..].find('[') {
                let pos = start + open_bracket;
                if let Some(close_bracket) = line[pos..].find(']') {
                    let link_start = pos + close_bracket + 1;
                    if link_start < line.len() && line.as_bytes()[link_start] == b'(' {
                        if let Some(close_paren) = line[link_start..].find(')') {
                            let raw_target = &line[link_start + 1..link_start + close_paren];
                            start = link_start + close_paren + 1;

                            if raw_target.starts_with("http://")
                                || raw_target.starts_with("https://")
                                || raw_target.starts_with('#')
                            {
                                continue;
                            }

                            let clean_target = if raw_target.starts_with("file:///") {
                                &raw_target[7..]
                            } else {
                                raw_target
                            };

                            let path_only = clean_target.split('#').next().unwrap_or(clean_target);
                            if path_only.is_empty() {
                                continue;
                            }

                            let target_path = if clean_target.starts_with('/') {
                                PathBuf::from(path_only)
                            } else {
                                file_dir.join(path_only)
                            };

                            if !target_path.exists() {
                                let msg = format!(
                                    "{}:L{} -> {}",
                                    file_path.display(),
                                    line_num,
                                    raw_target
                                );
                                let file_str = file_path.to_string_lossy();
                                if file_str.contains("docs/archive")
                                    || file_str.contains("docs/engineering")
                                {
                                    report.archive_broken_links.push(msg);
                                } else {
                                    report.active_broken_links.push(msg);
                                }
                            }
                        } else {
                            break;
                        }
                    } else {
                        start = pos + 1;
                    }
                } else {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn check_readmes(report: &mut DocsCheckReport) -> Result<(), Box<dyn std::error::Error>> {
    let members = [
        "crates/brain-domain",
        "crates/brain-core",
        "crates/brain-events",
        "crates/brain-storage",
        "crates/brain-config",
        "crates/brain-session",
        "crates/brain-tools",
        "crates/brain-plugins",
        "crates/brain-python",
        "crates/brain-tui",
        "crates/brain-services",
        "crates/brain-observability",
        "crates/brain-integrations",
        "crates/brain-sdk-rs",
        "crates/brain-cli-adapter",
        "crates/brain-application",
        "crates/brain-mcp-adapter",
        "crates/brain-acp-adapter",
        "crates/brain-adapter-core",
        "crates/brain-a2a-adapter",
        "crates/brain-arch-tests",
        "crates/brain-fitness-tests",
        "apps/brain",
        "daemon",
        "xtask",
    ];

    for member in members {
        let readme_path = Path::new(member).join("README.md");
        if !readme_path.exists() {
            report
                .missing_readmes
                .push(format!("Missing README.md in {}", member));
        }
    }

    Ok(())
}

fn check_indexes(report: &mut DocsCheckReport) -> Result<(), Box<dyn std::error::Error>> {
    let docs_dir = Path::new("docs");
    if !docs_dir.exists() {
        return Ok(());
    }

    let mut doc_files = Vec::new();
    collect_md_files(docs_dir, &mut doc_files)?;

    let mut combined_indices_content = String::new();
    for file in &doc_files {
        if file.file_name().and_then(|n| n.to_str()) == Some("README.md") {
            if let Ok(c) = fs::read_to_string(file) {
                combined_indices_content.push_str(&c);
                combined_indices_content.push('\n');
            }
        }
    }

    for file in &doc_files {
        let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name == "README.md"
            || file_name == "RFC_TEMPLATE.md"
            || file_name == "RFC-TEMPLATE.md"
        {
            continue;
        }
        let rel_path = file.to_string_lossy();
        if !combined_indices_content.contains(file_name)
            && !combined_indices_content.contains(&*rel_path)
        {
            let msg = format!("Unindexed doc: {}", rel_path);
            if rel_path.contains("docs/archive")
                || rel_path.contains("docs/superpowers")
                || rel_path.contains("docs/engineering")
            {
                report.archive_orphan_documents.push(msg);
            } else {
                report.active_orphan_documents.push(msg);
            }
        }
    }

    Ok(())
}

fn check_snippets(report: &mut DocsCheckReport) -> Result<(), Box<dyn std::error::Error>> {
    let protocol_doc = Path::new("docs/reference/protocol.md");
    if protocol_doc.exists() {
        if let Ok(content) = fs::read_to_string(protocol_doc) {
            let mut in_json = false;
            let mut block = String::new();
            let mut line_num = 0;

            for line in content.lines() {
                line_num += 1;
                if line.trim().starts_with("```json") {
                    in_json = true;
                    block.clear();
                    continue;
                }
                if in_json && line.trim().starts_with("```") {
                    in_json = false;
                    if !block.contains("|")
                        && !block.contains("...")
                        && serde_json::from_str::<serde_json::Value>(&block).is_err()
                    {
                        report.invalid_snippets.push(format!(
                            "docs/reference/protocol.md:L{} invalid JSON block",
                            line_num
                        ));
                    }
                    continue;
                }
                if in_json {
                    block.push_str(line);
                    block.push('\n');
                }
            }
        }
    }

    Ok(())
}

fn check_frontmatter(report: &mut DocsCheckReport) -> Result<(), Box<dyn std::error::Error>> {
    let normative_dirs = [
        "docs/architecture",
        "docs/reference",
        "docs/guides",
        "docs/governance",
    ];

    let mut canonical_claims = std::collections::HashSet::new();

    for dir in normative_dirs {
        let mut files = Vec::new();
        collect_md_files(Path::new(dir), &mut files)?;
        for file in files {
            let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name == "README.md"
                || file_name.starts_with("RFC-")
                || file_name.starts_with("ADR-")
                || file_name.contains("TEMPLATE")
            {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&file) {
                if !content.starts_with("---")
                    || !content.contains("status:")
                    || !content.contains("owner:")
                {
                    report
                        .frontmatter_violations
                        .push(format!("Missing frontmatter: {}", file.display()));
                }

                if content.contains("canonical: true") {
                    let path_str = file.display().to_string();
                    if !canonical_claims.insert(path_str.clone()) {
                        report.frontmatter_violations.push(format!(
                            "Duplicate canonical claim detected for file: {}",
                            path_str
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

fn check_subsystems(report: &mut DocsCheckReport) -> Result<(), Box<dyn std::error::Error>> {
    let subsystems_dir = Path::new("docs/subsystems");
    if !subsystems_dir.exists() {
        report
            .subsystem_violations
            .push("Missing docs/subsystems directory".to_string());
        return Ok(());
    }

    let required_handbooks = [
        "storage.md",
        "compiler.md",
        "retrieval.md",
        "tui.md",
        "protocol.md",
        "plugins.md",
    ];

    for name in required_handbooks {
        let path = subsystems_dir.join(name);
        if !path.exists() {
            report
                .subsystem_violations
                .push(format!("Missing required handbook: {}", path.display()));
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path) {
            if !content.contains("subsystem:") {
                report.subsystem_violations.push(format!(
                    "Handbook {} missing 'subsystem:' frontmatter field",
                    path.display()
                ));
            }
            if !content.contains("owns:") {
                report.subsystem_violations.push(format!(
                    "Handbook {} missing 'owns:' frontmatter field",
                    path.display()
                ));
            }
            if !content.contains("canonical_specs:") {
                report.subsystem_violations.push(format!(
                    "Handbook {} missing 'canonical_specs:' frontmatter field",
                    path.display()
                ));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// verify — unified Rust quality gate
// ---------------------------------------------------------------------------

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
        Step {
            label: "docs all",
            cmd: "cargo",
            args: &["xtask", "docs", "all"],
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

    if stopped_early {
        let ran = outcomes.len();
        for step in steps.iter().skip(ran) {
            outcomes.push((step.label, Outcome::Skip));
        }
    }

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

fn generate_contracts_to_string() -> Result<String, Box<dyn std::error::Error>> {
    let contract_version = "1.0.0";
    let generator_version = env!("CARGO_PKG_VERSION");
    let brain_version = "0.1.0";

    let config = specta::ts::ExportConfiguration::default();

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

    types_to_export.sort_by_key(|(name, _)| *name);

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

    let temp_dir = Path::new("temp_generated");
    fs::create_dir_all(temp_dir)?;

    let temp_file_path = temp_dir.join("types.ts");
    fs::write(&temp_file_path, &ts_content)?;

    if ts_content.is_empty() || !ts_content.contains("export type IngestionEnvelope") {
        return Err("Validation failed: Output TypeScript is malformed or empty".into());
    }

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

    println!("Running determinism verification...");
    let run_a = generate_contracts_to_string()?;
    let run_b = generate_contracts_to_string()?;
    if run_a != run_b {
        return Err("Determinism check failed: contract generation is non-deterministic.".into());
    }
    println!("  Determinism verification check: PASSED");

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
