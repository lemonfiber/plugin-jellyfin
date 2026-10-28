use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_pinned::Https;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Applied, Encrypting, Failure};
use serde_json::{json, Value};

use super::configured::{document, CONFIGURATION};
use super::declared::INTERNAL_TLS;
use super::Jellyfin;

const NETWORK: &str = "network";

const WHAT: &str = "the network settings";

const HANDSHAKE_WITHIN: Duration = Duration::from_secs(5);

const PIN_DIGITS: usize = 64;

pub(super) fn internal_tls(base: &str) -> Option<Https> {
    reqwest::Url::parse(base)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .and_then(|host| Https::parse(&format!("https://{host}:{INTERNAL_TLS}")))
}

fn pinned(pin: &str) -> bool {
    pin.len() == PIN_DIGITS
        && pin
            .bytes()
            .all(|digit| digit.is_ascii_digit() || (b'a'..=b'f').contains(&digit))
}

fn network() -> String {
    format!("{CONFIGURATION}/{NETWORK}")
}

fn serving(path: &str) -> [(&'static str, Value); 5] {
    [
        ("EnableHttps", json!(true)),
        ("RequireHttps", json!(false)),
        ("CertificatePath", json!(path)),
        ("CertificatePassword", json!("")),
        ("InternalHttpsPort", json!(INTERNAL_TLS)),
    ]
}

fn served(settings: &Value, path: &str) -> bool {
    serving(path)
        .iter()
        .all(|(field, value)| settings.get(*field) == Some(value))
}

async fn serve_tls(jellyfin: &Jellyfin, path: &str, pin: &str) -> Result<Applied, Failure> {
    if path.trim().is_empty() {
        return Err(jellyfin
            .endpoint
            .refused("no certificate path was given to serve the internal TLS port with"));
    }
    if !pinned(pin) {
        return Err(jellyfin.endpoint.refused(&format!(
            "{pin:?} is not a certificate's SHA-256 in lower-case hexadecimal"
        )));
    }
    let mut settings = document(jellyfin, &network(), WHAT).await?;
    if served(&settings, path) {
        let presented = match &jellyfin.internal_tls {
            Some(at) => lemonfiber_pinned::presents(at, pin, HANDSHAKE_WITHIN).await,
            None => false,
        };
        return Ok(if presented {
            Applied::Now
        } else {
            Applied::OnRestart
        });
    }
    let Some(fields) = settings.as_object_mut() else {
        return Err(jellyfin
            .endpoint
            .refused(&format!("{WHAT} are not an object")));
    };
    for (field, value) in serving(path) {
        fields.insert(field.to_owned(), value);
    }
    let response = jellyfin
        .as_admin(Method::Post, &network(), Some(settings.to_string()))
        .await?;
    jellyfin.endpoint.expect_success(&response)?;
    if !served(&document(jellyfin, &network(), WHAT).await?, path) {
        return Err(jellyfin.endpoint.refused(&format!(
            "{WHAT} were written to serve the internal TLS port with {path} and read back otherwise"
        )));
    }
    Ok(Applied::OnRestart)
}

#[async_trait]
impl Encrypting for Jellyfin {
    async fn serve_tls(&self, path: &str, pin: &str) -> Result<Applied, Failure> {
        serve_tls(self, path, pin).await
    }
}
