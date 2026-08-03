use std::fs;
use std::path::Path;

#[test]
fn test_no_hardcoded_colors_outside_theme_system() {
    let ui_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let theme_dir = ui_dir.join("theme");

    let mut violations = Vec::new();
    scan_dir_for_color_usage(&ui_dir, &theme_dir, &mut violations);

    if !violations.is_empty() {
        panic!(
            "\nArchitectural Violation: Hardcoded `Color::` usage detected in TUI widgets outside `src/ui/theme/`!\n\
            Widgets must use semantic `ThemeToken`s or `Theme` fields rather than hardcoded presentation colors.\n\n\
            Found {} violation(s):\n{}",
            violations.len(),
            violations.join("\n")
        );
    }
}

fn scan_dir_for_color_usage(dir: &Path, theme_dir: &Path, violations: &mut Vec<String>) {
    let entries = fs::read_dir(dir).expect("Failed to read directory");
    for entry in entries {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.is_dir() {
            if path.canonicalize().ok() == theme_dir.canonicalize().ok() {
                continue; // Skip src/ui/theme/ directory
            }
            scan_dir_for_color_usage(&path, theme_dir, violations);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            check_file_for_colors(&path, violations);
        }
    }
}

fn check_file_for_colors(path: &Path, violations: &mut Vec<String>) {
    let content = fs::read_to_string(path).expect("Failed to read file");
    let rel_path = path
        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(path)
        .display();

    for (line_idx, line) in content.lines().enumerate() {
        let line_trimmed = line.trim();
        if line_trimmed.starts_with("//") {
            continue;
        }

        if line.contains("Color::") || line.contains("ratatui::style::Color::") {
            violations.push(format!(
                "  - {}:L{}: {}",
                rel_path,
                line_idx + 1,
                line_trimmed
            ));
        }
    }
}
