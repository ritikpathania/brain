#[cfg(target_os = "macos")]
use std::path::Path;

fn main() {
    #[cfg(target_os = "macos")]
    {
        let candidate_framework_dirs = [
            "/Library/Developer/CommandLineTools/Library/Frameworks",
            "/Applications/Xcode.app/Contents/Developer/Library/Frameworks",
            "/System/Library/Frameworks",
        ];

        for framework_dir in candidate_framework_dirs {
            let p = Path::new(framework_dir);
            if p.exists() {
                println!("cargo:rustc-link-search=framework={}", framework_dir);
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", framework_dir);

                let versions_dir = p.join("Python3.framework").join("Versions");
                if versions_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&versions_dir) {
                        for entry in entries.flatten() {
                            let lib_path = entry.path().join("lib");
                            if lib_path.exists() {
                                println!("cargo:rustc-link-search=native={}", lib_path.display());
                                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
                            }
                        }
                    }
                }
            }
        }
    }
}
