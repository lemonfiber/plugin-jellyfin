use lemonfiber_ports::service::Failure;

use super::configured::{Listed, CONFIGURATION};
use super::Jellyfin;

const ORIGINS: Listed = Listed {
    document: CONFIGURATION,
    field: "CorsHosts",
    what: "the allow-list",
};

pub(super) async fn allowed_origins(jellyfin: &Jellyfin) -> Result<Vec<String>, Failure> {
    ORIGINS.read(jellyfin).await
}

pub(super) async fn allow_only(jellyfin: &Jellyfin, origin: &str) -> Result<(), Failure> {
    if origin.trim().is_empty() || origin.contains('*') {
        return Err(jellyfin.endpoint.refused(&format!(
            "{origin:?} is not an origin the allow-list may be written as"
        )));
    }
    ORIGINS.only(jellyfin, origin).await
}
