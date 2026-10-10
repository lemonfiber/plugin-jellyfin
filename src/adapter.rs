//! What this adapter says it is, and the kit that serves its contracts.

use std::sync::Arc;

use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::{identity, media};

use crate::jellyfin::Jellyfin;

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "Jellyfin";

/// The Jellyfin release this adapter's recordings were taken from.
pub const RELEASE: &str = "10.11.11";

/// The digest of the image that release's recordings were taken from.
pub const DIGEST: &str = "sha256:aefb67e6a7ff1debdd154a78a7bbb780fd0c873d8639210a7f6a2016ad2b35db";

/// What this adapter says it is: the contracts it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![
            lemonfiber_contract::spoken(media::serve::CAPABILITY, media::serve::MAJOR),
            lemonfiber_contract::spoken(identity::source::CAPABILITY, identity::source::MAJOR),
        ],
        upstream: UPSTREAM.to_owned(),
        releases: vec![Release {
            version: RELEASE.to_owned(),
            digest: DIGEST.to_owned(),
        }],
    }
}

/// Both contracts, served from `jellyfin` under `key`, ready while Jellyfin answers.
#[must_use]
pub fn kit(key: String, jellyfin: Arc<Jellyfin>) -> Kit {
    let asked = Arc::clone(&jellyfin);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.answers().await })
    });
    Kit::new(key, about())
        .ready_when(ready)
        .serving(media::serve::served(Arc::clone(&jellyfin)))
        .serving(identity::source::served(jellyfin))
}

#[cfg(test)]
mod tests;
