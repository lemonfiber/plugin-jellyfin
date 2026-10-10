use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::credential::Credential;

type Key = (String, Vec<u8>);

/// The tokens this adapter holds, by the credential that minted each.
#[derive(Debug, Default)]
pub struct Sessions(Mutex<BTreeMap<Key, String>>);

impl Sessions {
    pub(super) fn held(&self, credential: &Credential) -> Option<String> {
        self.0.lock().ok()?.get(&filed(credential)).cloned()
    }

    pub(super) fn keep(&self, credential: &Credential, token: &str) {
        if let Ok(mut held) = self.0.lock() {
            held.clear();
            held.insert(filed(credential), token.to_owned());
        }
    }

    pub(super) fn forget(&self, credential: &Credential) {
        if let Ok(mut held) = self.0.lock() {
            held.remove(&filed(credential));
        }
    }
}

fn filed(credential: &Credential) -> Key {
    let digest = ring::digest::digest(&ring::digest::SHA256, credential.password.as_bytes());
    (credential.username.clone(), digest.as_ref().to_vec())
}
