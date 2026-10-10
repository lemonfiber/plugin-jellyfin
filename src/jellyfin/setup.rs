use async_trait::async_trait;
use serde::Deserialize;

use super::Jellyfin;
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
