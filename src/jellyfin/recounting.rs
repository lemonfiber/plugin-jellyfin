use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Heard, Recounting};

use super::Jellyfin;

const NOT_YET: &str =
    "Jellyfin sends what happened only through its Webhook plugin, which this adapter does not set up yet";

#[async_trait]
impl Recounting for Jellyfin {
    async fn events(
        &self,
        _address: Option<&str>,
        _pin: Option<&str>,
        _key: Option<&str>,
        _every: Option<u32>,
    ) -> Result<Option<Heard>, Failure> {
        Err(self.endpoint.unsupported(NOT_YET))
    }
}
