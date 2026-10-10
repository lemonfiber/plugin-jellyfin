use serde::Deserialize;

use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Dated, Failure};

use super::{carrying, Jellyfin, AUTHORIZATION_HEADER};
use crate::endpoint::form_encoded;
use crate::shape::a_key;

const KEYS: &str = "/Auth/Keys";

pub(super) async fn filed_as(jellyfin: &Jellyfin, app: &str) -> Result<Vec<String>, Failure> {
    Ok(listed(jellyfin, app)
        .await?
        .into_iter()
        .map(|key| key.access_token)
        .collect())
}

pub(super) async fn dated(jellyfin: &Jellyfin, app: &str) -> Result<Vec<Dated>, Failure> {
    Ok(listed(jellyfin, app)
        .await?
        .into_iter()
        .map(|key| Dated {
            created: key.created,
            last_used: key.last_used,
        })
        .collect())
}

async fn listed(jellyfin: &Jellyfin, app: &str) -> Result<Vec<Key>, Failure> {
    let response = jellyfin.as_admin(Method::Get, KEYS, None).await?;
    let keys: Keys = jellyfin
        .endpoint
        .decode(&response, "the key list could not be read")?;
    Ok(keys
        .items
        .into_iter()
        .filter(|key| key.app_name == app && !key.access_token.is_empty())
        .collect())
}

pub(super) async fn mint(jellyfin: &Jellyfin, app: &str) -> Result<String, Failure> {
    let before = filed_as(jellyfin, app).await?;
    let asked = form_encoded(&[("App", app)]);
    let response = jellyfin
        .as_admin(Method::Post, &format!("{KEYS}?{asked}"), None)
        .await?;
    jellyfin.endpoint.expect_success(&response)?;
    let mut new: Vec<String> = filed_as(jellyfin, app)
        .await?
        .into_iter()
        .filter(|key| !before.contains(key))
        .collect();
    match (new.pop(), new.is_empty()) {
        (Some(key), true) => Ok(key),
        _ => Err(jellyfin.endpoint.refused(&format!(
            "a key was minted as {app} and the key list does not show exactly one new one"
        ))),
    }
}

pub(super) async fn answers_to(jellyfin: &Jellyfin, key: &str) -> Result<(), Failure> {
    if !a_key(key) {
        return Err(jellyfin.misshapen("a key"));
    }
    let mut asking = jellyfin.request(Method::Get, "/System/Info", None);
    asking
        .headers
        .push((AUTHORIZATION_HEADER.to_owned(), carrying(key)));
    let response = jellyfin.endpoint.send(&asking).await?;
    jellyfin.endpoint.expect_success(&response)
}

pub(super) async fn revoke(jellyfin: &Jellyfin, key: &str) -> Result<(), Failure> {
    if !a_key(key) {
        return Err(jellyfin.misshapen("a key"));
    }
    let response = jellyfin
        .as_admin(Method::Delete, &format!("{KEYS}/{key}"), None)
        .await?;
    jellyfin.endpoint.expect_success(&response)
}

#[derive(Deserialize)]
struct Keys {
    #[serde(rename = "Items", default)]
    items: Vec<Key>,
}

#[derive(Deserialize)]
struct Key {
    #[serde(rename = "AppName", default)]
    app_name: String,
    #[serde(rename = "AccessToken", default)]
    access_token: String,
    #[serde(rename = "DateCreated", default)]
    created: Option<String>,
    #[serde(rename = "DateLastActivity", default)]
    last_used: Option<String>,
}
