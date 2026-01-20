use std::env;
use std::path::PathBuf;

fn main() {
    #[cfg(target_os = "linux")]
    let cdylib_ext = "so";
    #[cfg(target_os = "macos")]
    let cdylib_ext = "dylib";

    let pkg_name = env::var("CARGO_PKG_NAME").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let target_dir = env::var("CARGO_TARGET_DIR");
    let profile = env::var("PROFILE").unwrap();

    let dylib_path = target_dir
        .map(|v| PathBuf::from(&v))
        .unwrap_or(PathBuf::from(&manifest_dir).join("target"))
        .join(&profile)
        .join(format!("lib{pkg_name}.{cdylib_ext}"));

    println!("cargo::rustc-env=DYLIB_PATH={}", dylib_path.display());
}
