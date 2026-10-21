//! Downloads and verifies whisper.cpp models into the app's data folder.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub size_bytes: u64,
    pub sha256: &'static str,
}

// Sizes and hashes come from the Hugging Face LFS pointers for ggerganov/whisper.cpp.
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "tiny.en",
        label: "Tiny (English, fastest)",
        size_bytes: 77_704_715,
        sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
    },
    ModelInfo {
        id: "base.en",
        label: "Base (English, recommended)",
        size_bytes: 147_964_211,
        sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
    },
    ModelInfo {
        id: "small.en",
        label: "Small (English, most accurate)",
        size_bytes: 487_614_201,
        sha256: "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
    },
];

pub fn find(id: &str) -> Result<&'static ModelInfo, String> {
    MODELS
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("unknown speech model {id:?}"))
}

pub fn path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("ggml-{id}.bin"))
}

/// Size check only: the full hash was verified when the file was downloaded.
pub fn is_installed(dir: &Path, model: &ModelInfo) -> bool {
    std::fs::metadata(path(dir, model.id))
        .map(|m| m.len() == model.size_bytes)
        .unwrap_or(false)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub id: &'static str,
    pub label: &'static str,
    pub size_bytes: u64,
    pub installed: bool,
}

pub fn statuses(dir: &Path) -> Vec<ModelStatus> {
    MODELS
        .iter()
        .map(|m| ModelStatus {
            id: m.id,
            label: m.label,
            size_bytes: m.size_bytes,
            installed: is_installed(dir, m),
        })
        .collect()
}

/// Streams `reader` to `dest` through a `.part` file, verifying size and SHA-256 before the
/// final rename, so a cancelled or corrupt download never looks like an installed model.
pub fn write_verified(
    mut reader: impl Read,
    dest: &Path,
    model: &ModelInfo,
    mut progress: impl FnMut(u64),
    cancel: &AtomicBool,
) -> Result<(), String> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = dest.with_extension("bin.part");
    let result = (|| {
        let mut out = BufWriter::new(File::create(&part).map_err(|e| e.to_string())?);
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024];
        let mut received = 0u64;
        let mut last_report = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("download cancelled".to_string());
            }
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            received += n as u64;
            if received - last_report >= 1 << 20 {
                last_report = received;
                progress(received);
            }
        }
        out.flush().map_err(|e| e.to_string())?;
        progress(received);
        if received != model.size_bytes {
            return Err(format!(
                "download incomplete: got {received} of {} bytes",
                model.size_bytes
            ));
        }
        let digest = hex(&hasher.finalize());
        if digest != model.sha256 {
            return Err(format!("checksum mismatch for {}: {digest}", model.id));
        }
        std::fs::rename(&part, dest).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn download(
    dir: &Path,
    id: &str,
    progress: impl FnMut(u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let model = find(id)?;
    let dest = path(dir, id);
    if is_installed(dir, model) {
        return Ok(dest);
    }
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{id}.bin");
    let response = ureq::get(&url)
        .call()
        .map_err(|e| format!("could not download {id}: {e}"))?;
    let reader = response.into_body().into_reader();
    write_verified(reader, &dest, model, progress, cancel)?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fake_model(data: &[u8]) -> ModelInfo {
        let sha: &'static str = Box::leak(hex(&Sha256::digest(data)).into_boxed_str());
        ModelInfo {
            id: "test",
            label: "Test",
            size_bytes: data.len() as u64,
            sha256: sha,
        }
    }

    #[test]
    fn verified_download_lands_at_dest_and_reports_progress() {
        let dir = tempfile::tempdir().unwrap();
        let data = vec![7u8; 3 * 1024 * 1024 + 11];
        let model = fake_model(&data);
        let dest = dir.path().join("models/ggml-test.bin");
        let mut reports = Vec::new();
        write_verified(
            Cursor::new(&data),
            &dest,
            &model,
            |n| reports.push(n),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert_eq!(*reports.last().unwrap(), data.len() as u64);
        assert!(reports.len() >= 3);
        assert!(!dest.with_extension("bin.part").exists());
    }

    #[test]
    fn checksum_mismatch_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let mut model = fake_model(b"expected bytes");
        model.sha256 = "0000";
        let dest = dir.path().join("ggml-test.bin");
        let err = write_verified(
            Cursor::new(b"expected bytes"),
            &dest,
            &model,
            |_| {},
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert!(err.contains("checksum"), "{err}");
        assert!(!dest.exists());
        assert!(!dest.with_extension("bin.part").exists());
    }

    #[test]
    fn short_download_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let model = fake_model(b"twelve bytes");
        let dest = dir.path().join("ggml-test.bin");
        let err = write_verified(
            Cursor::new(b"twelve"),
            &dest,
            &model,
            |_| {},
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert!(err.contains("incomplete"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn cancel_stops_the_download() {
        let dir = tempfile::tempdir().unwrap();
        let model = fake_model(b"data");
        let dest = dir.path().join("ggml-test.bin");
        let err = write_verified(
            Cursor::new(b"data"),
            &dest,
            &model,
            |_| {},
            &AtomicBool::new(true),
        )
        .unwrap_err();
        assert_eq!(err, "download cancelled");
        assert!(!dest.exists());
    }

    #[test]
    fn installed_check_uses_size() {
        let dir = tempfile::tempdir().unwrap();
        let model = find("tiny.en").unwrap();
        assert!(!is_installed(dir.path(), model));
        std::fs::write(path(dir.path(), "tiny.en"), b"too small").unwrap();
        assert!(!is_installed(dir.path(), model));
    }

    #[test]
    fn known_models_have_well_formed_hashes() {
        for m in MODELS {
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(m.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        }
        assert!(find("base.en").is_ok());
        assert!(find("large").is_err());
    }
}
