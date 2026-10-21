//! API keys for cloud providers live in the macOS Keychain, never in settings.json, and are
//! never sent back to the UI (it only learns whether a key is set).

use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

const SERVICE: &str = "com.eshaanjain.parley";
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

pub fn set(account: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("API key is empty".into());
    }
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
