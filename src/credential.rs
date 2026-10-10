//! The credential this adapter signs in to Jellyfin with, as lemonfiber writes it into the
//! adapter's configuration directory.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The file in the adapter's configuration directory lemonfiber writes the upstream's
/// credential to.
pub const UPSTREAM_FILE: &str = "upstream.json";

const LARGEST: u64 = 4096;

/// An account on Jellyfin and its password.
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct Credential {
    /// The account's name.
    pub username: String,
    /// Its password.
    pub password: String,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Credential")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

/// Where the credential a sign-in uses comes from.
pub trait Holding: Send + Sync {
    /// The credential held now.
    ///
    /// # Errors
    ///
    /// Why no credential is held, in words that name no part of one.
    fn credential(&self) -> Result<Credential, String>;
}

impl Holding for Credential {
    fn credential(&self) -> Result<Credential, String> {
        Ok(self.clone())
    }
}

/// The credential file in one configuration directory.
#[derive(Debug, Clone)]
pub struct Upstream(PathBuf);

impl Upstream {
    /// The credential file lemonfiber writes into `configuration`.
    #[must_use]
    pub fn in_directory(configuration: &Path) -> Self {
        Self(configuration.join(UPSTREAM_FILE))
    }
}

impl Holding for Upstream {
    fn credential(&self) -> Result<Credential, String> {
        let file = std::fs::File::open(&self.0)
            .map_err(|why| format!("{UPSTREAM_FILE} could not be opened: {why}"))?;
        let mut text = String::new();
        file.take(LARGEST + 1)
            .read_to_string(&mut text)
            .map_err(|why| format!("{UPSTREAM_FILE} could not be read: {why}"))?;
        if text.len() as u64 > LARGEST {
            return Err(format!("{UPSTREAM_FILE} is larger than {LARGEST} bytes"));
        }
        let held: Credential = serde_json::from_str(&text).map_err(|_| {
            format!("{UPSTREAM_FILE} is not an object naming a username and a password")
        })?;
        if held.username.is_empty() || held.password.is_empty() {
            return Err(format!(
                "{UPSTREAM_FILE} names an empty username or password"
            ));
        }
        Ok(held)
    }
}

#[cfg(test)]
mod tests;
