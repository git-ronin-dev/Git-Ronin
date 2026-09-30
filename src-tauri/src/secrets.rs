//! Account tokens in the OS keyring (Secret Service, macOS Keychain,
//! Windows Credential Manager). Without a usable keyring they are kept in
//! memory for the session only, and the user is told so.

use std::collections::HashMap;
use std::sync::Mutex;

use ronin_hosting::Credential;

use crate::state::lock;

const SERVICE: &str = "Git Ronin";

#[derive(Default)]
pub struct SecretStore {
    /// Read-through cache, and the only copy when the keyring failed.
    cache: Mutex<HashMap<String, Credential>>,
}

fn entry(id: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("account:{id}"))
}

impl SecretStore {
    /// The credential of account `id`.
    pub fn get(&self, id: &str) -> Result<Credential, String> {
        if let Some(credential) = lock(&self.cache).get(id) {
            return Ok(credential.clone());
        }
        let text = entry(id)
            .and_then(|e| e.get_password())
            .map_err(|e| match e {
                keyring::Error::NoEntry => "not signed in".to_owned(),
                e => format!("could not read the keyring: {e}"),
            })?;
        let credential: Credential = serde_json::from_str(&text)
            .map_err(|e| format!("the saved sign-in is damaged ({e}); sign in again"))?;
        lock(&self.cache).insert(id.to_owned(), credential.clone());
        Ok(credential)
    }

    pub fn has(&self, id: &str) -> bool {
        self.get(id).is_ok()
    }

    /// Saves the credential of account `id`. Returns a warning if it could
    /// only be kept until the app quits.
    pub fn set(&self, id: &str, credential: &Credential) -> Option<String> {
        lock(&self.cache).insert(id.to_owned(), credential.clone());
        let text = serde_json::to_string(credential).expect("credentials serialize");
        match entry(id).and_then(|e| e.set_password(&text)) {
            Ok(()) => None,
            Err(e) => Some(format!(
                "The token could not be saved in your system's keyring ({e}), so you'll need to \
                 sign in again after restarting Git Ronin."
            )),
        }
    }

    pub fn delete(&self, id: &str) {
        lock(&self.cache).remove(id);
        if let Ok(entry) = entry(id) {
            let _ = entry.delete_credential();
        }
    }
}
