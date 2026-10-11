//! Talking to Jellyfin, the media server, which lemonfiber holds an administrator's
//! account on rather than a key.

use std::sync::Arc;

use lemonfiber_ports::http::{Http, Method, Request, Response};
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::Failure;
use serde::Deserialize;

use crate::credential::{Credential, Holding};
use crate::endpoint::Endpoint;

mod configured;
mod cors;
mod household;
mod item;
mod keys;
mod library;
mod proxies;
mod screening;
mod serving;
mod sessions;
mod setup;
mod upkeep;

pub use sessions::Sessions;

/// What every failure is blamed on.
pub const SERVICE: &str = "jellyfin";

const AUTHORIZATION: &str =
    r#"MediaBrowser Client="lemonfiber", Device="lemonfiber", DeviceId="lemonfiber", Version="1""#;

const AUTHORIZATION_HEADER: &str = "Authorization";

const PUBLIC_INFO: &str = "/System/Info/Public";

fn carrying(token: &str) -> String {
    format!(r#"MediaBrowser Token="{token}""#)
}

fn carried(request: &Request, token: &str) -> Request {
    let mut carried = request.clone();
    carried
        .headers
        .push((AUTHORIZATION_HEADER.to_owned(), carrying(token)));
    carried
}

/// A client for one Jellyfin, signing in as the administrator whose credential `held`
/// supplies.
pub struct Jellyfin {
    endpoint: Endpoint,
    held: Arc<dyn Holding>,
    sessions: Sessions,
}

impl Jellyfin {
    /// The Jellyfin reached at `base` over `http`, signed in to with what `held` supplies.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, held: Arc<dyn Holding>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, SERVICE),
            held,
            sessions: Sessions::default(),
        }
    }

    /// Whether Jellyfin answers its public information.
    pub async fn answers(&self) -> bool {
        self.endpoint
            .send(&self.request(Method::Get, PUBLIC_INFO, None))
            .await
            .is_ok_and(|response| response.is_success())
    }

    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        self.endpoint.json_request(method, path, body)
    }

    fn credential(&self) -> Result<Credential, Failure> {
        self.held.credential().map_err(|why| {
            self.endpoint.refused(&format!(
                "this adapter holds no credential for Jellyfin: {why}"
            ))
        })
    }

    async fn as_admin(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<Response, Failure> {
        let credential = self.credential()?;
        let request = self.request(method, path, body);
        if let Some(token) = self.sessions.held(&credential) {
            let response = self.endpoint.send(&carried(&request, &token)).await?;
            if !matches!(response.status, 401 | 403) {
                return Ok(response);
            }
            self.sessions.forget(&credential);
        }
        let token = self.signed_in(&credential).await?.access_token;
        self.sessions.keep(&credential, &token);
        self.endpoint.send(&carried(&request, &token)).await
    }

    async fn signed_in(&self, credential: &Credential) -> Result<Session, Failure> {
        let body =
            serde_json::json!({ "Username": credential.username, "Pw": credential.password })
                .to_string();
        let mut request = self.request(Method::Post, "/Users/AuthenticateByName", Some(body));
        request
            .headers
            .push((AUTHORIZATION_HEADER.to_owned(), AUTHORIZATION.to_owned()));
        let response = self.endpoint.send(&request).await?;
        let session: Session = self
            .endpoint
            .decode(&response, "the sign-in was not accepted")?;
        if session.access_token.is_empty() {
            return Err(self.endpoint.refused("the sign-in opened no session"));
        }
        Ok(session)
    }

    fn misshapen(&self, what: &str) -> Failure {
        self.endpoint
            .refused(&format!("that is not {what} Jellyfin could hold"))
    }
}

#[derive(Deserialize)]
struct Session {
    #[serde(rename = "AccessToken", default)]
    access_token: String,
    #[serde(rename = "User", default)]
    user: SessionUser,
}

#[derive(Deserialize, Default)]
struct SessionUser {
    #[serde(rename = "Id", default)]
    id: String,
}

const fn item_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Tv => "Series",
        Kind::Movies => "Movie",
    }
}
