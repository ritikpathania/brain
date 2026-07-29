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

                let py3_lib = p
                    .join("Python3.framework")
                    .join("Versions")
                    .join("3.9")
                    .join("lib");
                if py3_lib.exists() {
                    println!("cargo:rustc-link-search=native={}", py3_lib.display());
                    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", py3_lib.display());
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
