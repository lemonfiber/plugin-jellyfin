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
    allow_origins(jellyfin, &[origin.to_owned()]).await
}

pub(super) async fn allow_origins(jellyfin: &Jellyfin, origins: &[String]) -> Result<(), Failure> {
    if origins.is_empty() {
        return Err(jellyfin
            .endpoint
            .refused("an allow-list of no origin would let every browser in"));
    }
    if let Some(origin) = origins
        .iter()
        .find(|origin| origin.trim().is_empty() || origin.contains('*'))
    {
        return Err(jellyfin.endpoint.refused(&format!(
            "{origin:?} is not an origin the allow-list may be written as"
        )));
    }
    ORIGINS.exactly(jellyfin, origins).await
}
