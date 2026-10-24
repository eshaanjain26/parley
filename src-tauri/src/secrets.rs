//! Secrets: the database key and API keys for cloud providers. Never in settings.json, and
//! never sent back to the UI (it only learns whether a key is set).
//!
//! Release builds keep them in the macOS Keychain. Debug builds keep them in a file only this
//! user can read (`dev-secrets.json`, mode 0600): self-signed dev builds have no Team ID, so
//! macOS ties each Keychain approval to one binary and asks again after every rebuild. A debug
//! build moves each existing Keychain secret into the file the first time it needs it.

use std::path::PathBuf;
use std::sync::OnceLock;

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Where debug builds keep the secrets file. Call once at startup, before any secret is read.
pub fn init(app_data_dir: PathBuf) {
    let _ = DIR.set(app_data_dir);
}

pub fn set(account: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("API key is empty".into());
    }
    #[cfg(debug_assertions)]
    {
        dev::file()?.set(account, key)
    }
    #[cfg(not(debug_assertions))]
    {
        keychain::set(account, key)
    }
}

pub fn get(account: &str) -> Result<Option<String>, String> {
    #[cfg(debug_assertions)]
    {
        dev::file()?.get_or_migrate(account, keychain::get)
    }
    #[cfg(not(debug_assertions))]
    {
        keychain::get(account)
    }
}

pub fn delete(account: &str) -> Result<(), String> {
    #[cfg(debug_assertions)]
    {
        dev::file()?.delete(account)?;
    }
    // Also clear any Keychain copy, so a debug build can't migrate a deleted key back.
    keychain::delete(account)
}

mod keychain {
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    const SERVICE: &str = "com.eshaanjain.parley";
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

    #[cfg_attr(debug_assertions, allow(dead_code))]
    pub fn set(account: &str, key: &str) -> Result<(), String> {
        set_generic_password(SERVICE, account, key.as_bytes())
            .map_err(|e| format!("could not save the key to the Keychain: {e}"))
    }

    pub fn get(account: &str) -> Result<Option<String>, String> {
        match get_generic_password(SERVICE, account) {
            Ok(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| "stored key is not valid text".into()),
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
            Err(e) => Err(format!("could not read the key from the Keychain: {e}")),
        }
    }

    pub fn delete(account: &str) -> Result<(), String> {
        match delete_generic_password(SERVICE, account) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            Err(e) => Err(format!("could not remove the key from the Keychain: {e}")),
        }
    }
}

/// The debug-build secrets file. Compiled in tests too, so it can be tested in any build.
#[cfg_attr(not(any(debug_assertions, test)), allow(dead_code))]
mod dev {
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::Mutex;

    pub const FILE_NAME: &str = "dev-secrets.json";

    pub struct SecretFile {
        path: PathBuf,
        // Serialises read-modify-write cycles between threads.
        lock: Mutex<()>,
    }

    pub fn file() -> Result<&'static SecretFile, String> {
        static FILE: std::sync::OnceLock<SecretFile> = std::sync::OnceLock::new();
        let dir = super::DIR
            .get()
            .ok_or("secrets used before secrets::init")?;
        Ok(FILE.get_or_init(|| SecretFile::new(dir.join(FILE_NAME))))
    }

    impl SecretFile {
        pub fn new(path: PathBuf) -> Self {
            Self {
                path,
                lock: Mutex::new(()),
            }
        }

        fn read(&self) -> Result<BTreeMap<String, String>, String> {
            match fs::read(&self.path) {
                Ok(bytes) => serde_json::from_slice(&bytes)
                    .map_err(|e| format!("{} is unreadable: {e}", self.path.display())),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
                Err(e) => Err(e.to_string()),
            }
        }

        /// Writes a 0600 temp file and renames it into place, so the file is never readable by
        /// others and never half-written.
        fn write(&self, map: &BTreeMap<String, String>) -> Result<(), String> {
            if let Some(dir) = self.path.parent() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let tmp = self.path.with_extension("json.tmp");
            let _ = fs::remove_file(&tmp);
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)
                .map_err(|e| e.to_string())?;
            f.write_all(&serde_json::to_vec_pretty(map).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
            fs::rename(&tmp, &self.path).map_err(|e| e.to_string())?;
            // An older file may have been created with looser permissions.
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())
        }

        pub fn set(&self, account: &str, value: &str) -> Result<(), String> {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let mut map = self.read()?;
            map.insert(account.into(), value.into());
            self.write(&map)
        }

        /// Reads from the file; if the account isn't there, asks `fallback` (the Keychain) once
        /// and keeps what it returns in the file.
        pub fn get_or_migrate(
            &self,
            account: &str,
            fallback: impl FnOnce(&str) -> Result<Option<String>, String>,
        ) -> Result<Option<String>, String> {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let mut map = self.read()?;
            if let Some(v) = map.get(account) {
                return Ok(Some(v.clone()));
            }
            let Some(value) = fallback(account)? else {
                return Ok(None);
            };
            map.insert(account.into(), value.clone());
            self.write(&map)?;
            Ok(Some(value))
        }

        pub fn delete(&self, account: &str) -> Result<(), String> {
            let _guard = self.lock.lock().map_err(|e| e.to_string())?;
            let mut map = self.read()?;
            if map.remove(account).is_some() {
                self.write(&map)?;
            }
            Ok(())
        }

        #[cfg(test)]
        pub fn path(&self) -> &std::path::Path {
            &self.path
        }
    }
}

#[cfg(test)]
mod tests {
    use super::dev::{FILE_NAME, SecretFile};
    use std::cell::Cell;
    use std::os::unix::fs::PermissionsExt;

    fn file() -> (tempfile::TempDir, SecretFile) {
        let dir = tempfile::tempdir().unwrap();
        let f = SecretFile::new(dir.path().join("nested").join(FILE_NAME));
        (dir, f)
    }

    #[test]
    fn stores_secrets_in_a_file_only_the_user_can_read() {
        let (_dir, f) = file();
        f.set("anthropic", "sk-test").unwrap();
        f.set("database-key", "abcd").unwrap();
        let mode = std::fs::metadata(f.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let never = |_: &str| -> Result<Option<String>, String> { panic!("no fallback expected") };
        assert_eq!(
            f.get_or_migrate("anthropic", never).unwrap().as_deref(),
            Some("sk-test")
        );
        assert!(!f.path().with_extension("json.tmp").exists());
    }

    #[test]
    fn migrates_a_keychain_secret_once_then_reads_the_file() {
        let (_dir, f) = file();
        let calls = Cell::new(0);
        let keychain = |_: &str| {
            calls.set(calls.get() + 1);
            Ok(Some("from-keychain".to_string()))
        };
        assert_eq!(
            f.get_or_migrate("database-key", keychain)
                .unwrap()
                .as_deref(),
            Some("from-keychain")
        );
        assert_eq!(
            f.get_or_migrate("database-key", keychain)
                .unwrap()
                .as_deref(),
            Some("from-keychain")
        );
        assert_eq!(calls.get(), 1, "the Keychain is asked only the first time");
    }

    #[test]
    fn missing_everywhere_is_none_and_keychain_errors_surface() {
        let (_dir, f) = file();
        assert_eq!(f.get_or_migrate("openai", |_| Ok(None)).unwrap(), None);
        assert!(!f.path().exists(), "nothing is written for a missing key");
        let err = f.get_or_migrate("openai", |_| Err("denied".to_string()));
        assert_eq!(err.unwrap_err(), "denied");
    }

    #[test]
    fn delete_removes_only_that_secret_and_tightens_old_permissions() {
        let (_dir, f) = file();
        f.set("a", "1").unwrap();
        f.set("b", "2").unwrap();
        std::fs::set_permissions(f.path(), std::fs::Permissions::from_mode(0o644)).unwrap();
        f.delete("a").unwrap();
        f.delete("missing").unwrap();
        assert_eq!(f.get_or_migrate("a", |_| Ok(None)).unwrap(), None);
        assert_eq!(
            f.get_or_migrate("b", |_| Ok(None)).unwrap().as_deref(),
            Some("2")
        );
        let mode = std::fs::metadata(f.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn a_corrupt_file_is_an_error_not_an_empty_store() {
        let (_dir, f) = file();
        std::fs::create_dir_all(f.path().parent().unwrap()).unwrap();
        std::fs::write(f.path(), "{not json").unwrap();
        assert!(f.get_or_migrate("x", |_| Ok(None)).is_err());
    }
}
