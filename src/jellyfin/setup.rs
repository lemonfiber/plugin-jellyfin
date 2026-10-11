use async_trait::async_trait;
use serde::Deserialize;

use super::{carried, Jellyfin};
use crate::credential::Credential;
use crate::shape::an_id;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, MediaServer};

#[derive(Deserialize)]
struct PublicInfo {
    #[serde(rename = "StartupWizardCompleted", default)]
    startup_wizard_completed: bool,
}

#[async_trait]
impl MediaServer for Jellyfin {
    async fn startup_completed(&self) -> Result<bool, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, "/System/Info/Public", None))
            .await?;
        let info: PublicInfo = self
            .endpoint
            .decode(&response, "the public system info could not be read")?;
        Ok(info.startup_wizard_completed)
    }

    async fn replace_admin_password(&self, replacement: &str) -> Result<(), Failure> {
        replaced(self, replacement).await
    }

    async fn create_admin(&self, name: &str, password: &str) -> Result<(), Failure> {
        let waiting = self
            .endpoint
            .send(&self.request(Method::Get, "/Startup/User", None))
            .await?;
        self.endpoint.expect_success(&waiting)?;

        let body = serde_json::json!({ "Name": name, "Password": password }).to_string();
        let created = self
            .endpoint
            .send(&self.request(Method::Post, "/Startup/User", Some(body)))
            .await?;
        self.endpoint.expect_success(&created)?;

        let completed = self
            .endpoint
            .send(&self.request(Method::Post, "/Startup/Complete", None))
            .await?;
        self.endpoint.expect_success(&completed)
    }
}

async fn replaced(jellyfin: &Jellyfin, replacement: &str) -> Result<(), Failure> {
    if replacement.is_empty() {
        return Err(jellyfin
            .endpoint
            .refused("no replacement password was given"));
    }
    let in_force = jellyfin.credential()?;
    let session = jellyfin.signed_in(&in_force).await?;
    if !an_id(&session.user.id) {
        return Err(jellyfin
            .endpoint
            .refused("the sign-in named no account Jellyfin could hold"));
    }
    let body =
        serde_json::json!({ "CurrentPw": in_force.password, "NewPw": replacement }).to_string();
    let changing = carried(
        &jellyfin.request(
            Method::Post,
            &format!("/Users/{}/Password", session.user.id),
            Some(body),
        ),
        &session.access_token,
    );
    let response = jellyfin.endpoint.send(&changing).await?;
    jellyfin.endpoint.expect_success(&response)?;
    let proving = Credential {
        username: in_force.username,
        password: replacement.to_owned(),
    };
    match jellyfin.signed_in(&proving).await {
        Ok(_) => Ok(()),
        Err(Failure::Unauthorised { .. }) => Err(jellyfin
            .endpoint
            .refused("Jellyfin did not take the new password at a sign-in")),
        Err(failure) => Err(failure),
    }
}
