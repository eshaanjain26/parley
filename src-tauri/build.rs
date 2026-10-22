use std::{env, path::PathBuf, process::Command};

fn main() {
    build_sidecar("parley-audiotap", "sidecar/audiotap/main.swift");
    build_sidecar("parley-mic", "sidecar/mic/main.swift");
    build_sidecar("parley-micwatch", "sidecar/micwatch/main.swift");
    build_sidecar("parley-calendar", "sidecar/calendar/main.swift");
    tauri_build::build()
}

/// Compiles a Swift sidecar into `binaries/` under the target-triple name Tauri's `externalBin`
/// expects. Must run before `tauri_build::build()`, which checks the binaries exist.
fn build_sidecar(name: &str, source: &str) {
    let target = env::var("TARGET").expect("cargo sets TARGET");
    if !target.ends_with("apple-darwin") {
        return;
    }
    let manifest =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let src = manifest.join(source);
    println!("cargo:rerun-if-changed={}", src.display());

    let out_dir = manifest.join("binaries");
    std::fs::create_dir_all(&out_dir).expect("create binaries dir");
    let out = out_dir.join(format!("{name}-{target}"));
    let arch = if target.starts_with("aarch64") {
        "arm64"
    } else {
        "x86_64"
    };

    // cargo can run this script for several profiles at once (clippy + test), so each run
    // compiles to its own file and renames it into place.
    let tmp = out_dir.join(format!("{name}-{target}.{}.tmp", std::process::id()));
    let status = Command::new("xcrun")
        .args([
            "swiftc",
            "-O",
            // The sidecars are top-level scripts; pin the language mode so a toolchain that
            // defaults to Swift 6 strict concurrency doesn't break the build.
            "-swift-version",
            "5",
            "-target",
            &format!("{arch}-apple-macos14.4"),
        ])
        .arg(&src)
        .arg("-o")
        .arg(&tmp)
        .status()
        .expect("run xcrun swiftc (install Xcode Command Line Tools)");
    assert!(status.success(), "compiling the {name} sidecar failed");
    std::fs::rename(&tmp, &out).unwrap_or_else(|e| panic!("move {name} into place: {e}"));
}
