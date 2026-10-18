use std::{env, path::PathBuf, process::Command};

fn main() {
    build_audiotap();
    tauri_build::build()
}

/// Compiles the Swift system-audio sidecar into `binaries/` under the target-triple name
/// Tauri's `externalBin` expects. Must run before `tauri_build::build()`, which checks it exists.
fn build_audiotap() {
    let target = env::var("TARGET").expect("cargo sets TARGET");
    if !target.ends_with("apple-darwin") {
        return;
    }
    let manifest =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let src = manifest.join("sidecar/audiotap/main.swift");
    println!("cargo:rerun-if-changed={}", src.display());

    let out_dir = manifest.join("binaries");
    std::fs::create_dir_all(&out_dir).expect("create binaries dir");
    let out = out_dir.join(format!("parley-audiotap-{target}"));
    let arch = if target.starts_with("aarch64") {
        "arm64"
    } else {
        "x86_64"
    };

    // cargo can run this script for several profiles at once (clippy + test), so each run
    // compiles to its own file and renames it into place.
    let tmp = out_dir.join(format!(
        "parley-audiotap-{target}.{}.tmp",
        std::process::id()
    ));
    let status = Command::new("xcrun")
        .args([
            "swiftc",
            "-O",
            "-target",
            &format!("{arch}-apple-macos14.4"),
        ])
        .arg(&src)
        .arg("-o")
        .arg(&tmp)
        .status()
        .expect("run xcrun swiftc (install Xcode Command Line Tools)");
    assert!(status.success(), "compiling the audiotap sidecar failed");
    std::fs::rename(&tmp, &out).expect("move audiotap sidecar into place");
}
