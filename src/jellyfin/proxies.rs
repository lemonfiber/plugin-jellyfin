use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::Failure;

use super::configured::Listed;
use super::Jellyfin;

const PROXIES: Listed = Listed {
    document: "/System/Configuration/network",
    field: "KnownProxies",
    what: "the trusted addresses",
};

const RESTART: &str = "/System/Restart";

pub(super) async fn known_proxies(jellyfin: &Jellyfin) -> Result<Vec<String>, Failure> {
    PROXIES.read(jellyfin).await
}

pub(super) async fn trust_only(jellyfin: &Jellyfin, address: &str) -> Result<(), Failure> {
    if address.trim().is_empty() {
        return Err(jellyfin.endpoint.refused("no address was given to trust"));
    }
    PROXIES.only(jellyfin, address).await
}

pub(super) async fn restart(jellyfin: &Jellyfin) -> Result<(), Failure> {
    let response = jellyfin.as_admin(Method::Post, RESTART, None).await?;
    jellyfin.endpoint.expect_success(&response)
}
