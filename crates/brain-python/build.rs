#[cfg(target_os = "macos")]
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=PYO3_PYTHON");
    println!("cargo:rerun-if-env-changed=LIBRARY_PATH");
    println!("cargo:rerun-if-changed=build.rs");

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

        let homebrew_dirs = [
            "/opt/homebrew/opt/python@3.9/lib",
            "/opt/homebrew/opt/python/lib",
            "/usr/local/opt/python@3.9/lib",
            "/usr/local/opt/python/lib",
        ];

        for hb_dir in homebrew_dirs {
            if Path::new(hb_dir).exists() {
                println!("cargo:rustc-link-search=native={}", hb_dir);
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", hb_dir);
            }
        }
    }
}
