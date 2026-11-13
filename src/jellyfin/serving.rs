use async_trait::async_trait;

use super::{cors, keys, proxies, Jellyfin};
use lemonfiber_ports::service::{
    Allowing, AppKeys, Astray, Dated, Failure, Fronted, OutsideAccount, OutsideServer,
};

#[async_trait]
impl Fronted for Jellyfin {
    async fn known_proxies(&self) -> Result<Vec<String>, Failure> {
        proxies::known_proxies(self).await
    }

    async fn trust_only(&self, address: &str) -> Result<(), Failure> {
        proxies::trust_only(self, address).await
    }

    async fn allowed_origins(&self) -> Result<Vec<String>, Failure> {
        cors::allowed_origins(self).await
    }

    async fn allow_only(&self, origin: &str) -> Result<(), Failure> {
        cors::allow_only(self, origin).await
    }

    async fn restart(&self) -> Result<(), Failure> {
        proxies::restart(self).await
    }
}

#[async_trait]
impl Allowing for Jellyfin {
    async fn allow_origins(&self, origins: &[String]) -> Result<(), Failure> {
        cors::allow_origins(self, origins).await
    }
}

#[async_trait]
impl AppKeys for Jellyfin {
    async fn filed_as(&self, app: &str) -> Result<Vec<String>, Failure> {
        keys::filed_as(self, app).await
    }

    async fn dated(&self, app: &str) -> Result<Vec<Dated>, Failure> {
        keys::dated(self, app).await
    }

    async fn mint(&self, app: &str) -> Result<String, Failure> {
        keys::mint(self, app).await
    }

    async fn answers_to(&self, key: &str) -> Result<(), Failure> {
        keys::answers_to(self, key).await
    }

    async fn revoke(&self, key: &str) -> Result<(), Failure> {
        keys::revoke(self, key).await
    }

    async fn astray(&self, app: &str) -> Result<Vec<Astray>, Failure> {
        keys::astray(self, app).await
    }

    async fn account(&self, _app: &str) -> Result<OutsideAccount, Failure> {
        Err(keys::held_itself(self))
    }

    async fn listed_as(&self) -> Result<OutsideServer, Failure> {
        Err(keys::held_itself(self))
    }
}
