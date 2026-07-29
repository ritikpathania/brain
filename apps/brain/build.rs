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
            if Path::new(framework_dir).exists() {
                println!("cargo:rustc-link-search=framework={}", framework_dir);
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", framework_dir);

                let py3_lib = Path::new(framework_dir)
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
    }
}
