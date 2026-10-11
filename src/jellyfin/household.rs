mod playing;
mod policy;
mod shelf;

use async_trait::async_trait;

use super::{item_type, Jellyfin};
use crate::endpoint::form_encoded;
use crate::shape::{a_device, a_key, an_id};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    Access, Allowed, Certificate, Failure, Household, Invited, Item, Medium, Member, NamedLibrary,
    Playback, Session, Signed, Unrated,
};

const SESSIONS: &str = "/Sessions";

const METADATA_COUNTRY: &str = "MetadataCountryCode";

#[derive(serde::Deserialize)]
struct UserResource {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "HasPassword", default)]
    has_password: bool,
    #[serde(rename = "Policy", default)]
    policy: Option<PolicyResource>,
    #[serde(rename = "LastActivityDate", default)]
    last_activity: Option<String>,
    #[serde(rename = "LastLoginDate", default)]
    last_login: Option<String>,
}

#[derive(serde::Deserialize, Default)]
struct PolicyResource {
    #[serde(rename = "EnableAllFolders", default)]
    every_library: bool,
    #[serde(rename = "EnabledFolders", default)]
    libraries: Vec<String>,
    #[serde(rename = "MaxParentalRating", default)]
    age_limit: Option<u32>,
    #[serde(rename = "BlockUnratedItems", default)]
    unrated_blocked: Vec<String>,
    #[serde(rename = "IsAdministrator", default)]
    administrator: bool,
    #[serde(rename = "IsDisabled", default)]
    disabled: bool,
}

impl UserResource {
    fn member(self) -> Member {
        let policy = self.policy.unwrap_or_default();
        Member {
            id: self.id,
            name: self.name,
            claimed: self.has_password,
            access: Access {
                every_library: policy.every_library,
                libraries: policy.libraries,
                age_limit: policy.age_limit,
                unrated: if policy.unrated_blocked.is_empty() {
                    Unrated::LetThrough
                } else {
                    Unrated::HeldBack
                },
                administrator: policy.administrator,
                disabled: policy.disabled,
            },
            last_seen: self.last_activity.or(self.last_login),
        }
    }
}

#[derive(serde::Deserialize)]
struct SessionResource {
    #[serde(rename = "UserId", default)]
    user: String,
    #[serde(rename = "DeviceId", default)]
    device_id: String,
    #[serde(rename = "DeviceName", default)]
    device: String,
    #[serde(rename = "Client", default)]
    client: String,
    #[serde(rename = "LastActivityDate", default)]
    last_activity: Option<String>,
}

#[derive(serde::Deserialize)]
struct AccountResource {
    #[serde(rename = "Policy", default)]
    policy: serde_json::Map<String, serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct RatingResource {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Value", default)]
    age: Option<u32>,
}

#[derive(serde::Deserialize)]
struct FolderResource {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
}

#[derive(serde::Deserialize)]
struct FoldersResource {
    #[serde(rename = "Items", default)]
    items: Vec<FolderResource>,
}

#[derive(serde::Deserialize)]
struct EntryResource {
    #[serde(rename = "Type", default)]
    kind: String,
    #[serde(rename = "Date", default)]
    date: String,
    #[serde(rename = "UserId", default)]
    user: String,
}

#[derive(serde::Deserialize)]
struct ActivityResource {
    #[serde(rename = "Items", default)]
    items: Vec<EntryResource>,
}

const RATINGS: &str = "/Localization/ParentalRatings";

const ACCOUNT_MADE: &str = "UserCreated";

const PASSWORD_MOVED: &str = "UserPasswordChanged";

const AT_ONCE: u32 = 200;

#[async_trait]
impl Household for Jellyfin {
    async fn household(&self) -> Result<Vec<Member>, Failure> {
        let response = self.as_admin(Method::Get, "/Users", None).await?;
        let held: Vec<UserResource> = self
            .endpoint
            .decode(&response, "the household's accounts could not be read")?;
        Ok(held.into_iter().map(UserResource::member).collect())
    }

    async fn whoever(
        &self,
        name: &str,
        password: &str,
        device: &str,
    ) -> Result<Option<Signed>, Failure> {
        whoever(self, name, password, device).await
    }

    async fn standing(&self, signed: &Signed) -> Result<bool, Failure> {
        standing(self, signed).await
    }

    async fn invite(&self, name: &str) -> Result<Member, Failure> {
        let body = serde_json::json!({ "Name": name }).to_string();
        let response = self
            .as_admin(Method::Post, "/Users/New", Some(body))
            .await?;
        let made: UserResource = self
            .endpoint
            .decode(&response, "the account could not be read back")?;
        Ok(made.member())
    }

    async fn claim(&self, name: &str, password: &str, device: &str) -> Result<bool, Failure> {
        claim(self, name, password, device).await
    }

    async fn unclaim(&self, id: &str) -> Result<(), Failure> {
        if !an_id(id) {
            return Err(self.misshapen("an account"));
        }
        let body = serde_json::json!({ "ResetPassword": true }).to_string();
        let response = self
            .as_admin(Method::Post, &format!("/Users/{id}/Password"), Some(body))
            .await?;
        self.endpoint.expect_success(&response)
    }

    async fn withdraw(&self, id: &str) -> Result<(), Failure> {
        if !an_id(id) {
            return Err(self.misshapen("an account"));
        }
        let response = self
            .as_admin(Method::Delete, &format!("/Users/{id}"), None)
            .await?;
        self.endpoint.expect_success(&response)
    }

    async fn when_invited(&self, since: &str) -> Result<Vec<Invited>, Failure> {
        let asked = form_encoded(&[("minDate", since), ("limit", &AT_ONCE.to_string())]);
        let path = format!("/System/ActivityLog/Entries?{asked}");
        let response = self.as_admin(Method::Get, &path, None).await?;
        let recorded: ActivityResource = self.endpoint.decode(
            &response,
            "what the media server recorded could not be read",
        )?;
        Ok(recorded
            .items
            .into_iter()
            .filter(|entry| {
                (entry.kind == ACCOUNT_MADE || entry.kind == PASSWORD_MOVED)
                    && !entry.user.is_empty()
            })
            .map(|entry| Invited {
                member: entry.user,
                at: entry.date,
            })
            .collect())
    }

    async fn ratings(&self) -> Result<Vec<Certificate>, Failure> {
        let response = self.as_admin(Method::Get, RATINGS, None).await?;
        let held: Vec<RatingResource> = self
            .endpoint
            .decode(&response, "the server's own ratings could not be read")?;
        Ok(held
            .into_iter()
            .filter_map(|rating| {
                rating.age.map(|age| Certificate {
                    name: rating.name,
                    age,
                })
            })
            .collect())
    }

    async fn country(&self) -> Result<Option<String>, Failure> {
        let configuration =
            super::configured::document(self, super::configured::CONFIGURATION, "the country")
                .await?;
        Ok(configuration
            .get(METADATA_COUNTRY)
            .and_then(serde_json::Value::as_str)
            .filter(|code| code.len() == 2 && code.bytes().all(|byte| byte.is_ascii_alphabetic()))
            .map(str::to_ascii_uppercase))
    }

    async fn allow(&self, id: &str, allowed: &Allowed) -> Result<(), Failure> {
        policy::rewritten(self, id, &policy::Edit::Allow(allowed)).await
    }

    async fn claimable(&self, id: &str, allowed: &Allowed) -> Result<(), Failure> {
        policy::rewritten(self, id, &policy::Edit::Claimable(allowed)).await
    }

    async fn suspend(&self, id: &str) -> Result<(), Failure> {
        policy::rewritten(self, id, &policy::Edit::Suspend).await
    }

    async fn sessions(&self, member: &str) -> Result<Vec<Session>, Failure> {
        let response = self.as_admin(Method::Get, SESSIONS, None).await?;
        let listed: Vec<SessionResource> = self
            .endpoint
            .decode(&response, "the media server's sessions could not be read")?;
        Ok(listed
            .into_iter()
            .filter(|session| session.user == member)
            .map(|session| Session {
                device_id: session.device_id,
                device: session.device,
                client: session.client,
                last_seen: session.last_activity,
            })
            .collect())
    }

    async fn signs_devices_in(&self) -> Result<bool, Failure> {
        let response = self
            .as_admin(Method::Get, "/QuickConnect/Enabled", None)
            .await?;
        self.endpoint.decode(
            &response,
            "the media server would not say whether it signs devices in by code",
        )
    }

    async fn libraries(&self) -> Result<Vec<NamedLibrary>, Failure> {
        let response = self
            .as_admin(Method::Get, "/Library/MediaFolders", None)
            .await?;
        let held: FoldersResource = self
            .endpoint
            .decode(&response, "the server's libraries could not be read")?;
        Ok(held
            .items
            .into_iter()
            .map(|folder| NamedLibrary {
                id: folder.id,
                name: folder.name,
            })
            .collect())
    }
}

async fn standing(jellyfin: &Jellyfin, signed: &Signed) -> Result<bool, Failure> {
    if !a_key(&signed.token) {
        return Ok(false);
    }
    let mut request = jellyfin.request(Method::Get, "/Users/Me", None);
    request.headers.push((
        super::AUTHORIZATION_HEADER.to_owned(),
        super::carrying(&signed.token),
    ));
    let response = jellyfin.endpoint.send(&request).await?;

    if response.status == WITHDRAWN {
        return Ok(false);
    }

    let held: UserResource = jellyfin.endpoint.decode(
        &response,
        "whether the account still stands could not be read",
    )?;

    Ok(held.id == signed.id
        && held.has_password
        && held.policy.is_none_or(|policy| !policy.disabled))
}

const WITHDRAWN: u16 = 401;

async fn whoever(
    jellyfin: &Jellyfin,
    name: &str,
    password: &str,
    device: &str,
) -> Result<Option<Signed>, Failure> {
    if !a_device(device) {
        return Err(jellyfin.misshapen("a device"));
    }
    let body = serde_json::json!({ "Username": name, "Pw": password }).to_string();
    let mut request = jellyfin.request(Method::Post, "/Users/AuthenticateByName", Some(body));
    request.headers.push((
        super::AUTHORIZATION_HEADER.to_owned(),
        format!(
            r#"MediaBrowser Client="lemonfiber", Device="lemonfiber", DeviceId="{device}", Version="1""#
        ),
    ));
    let response = jellyfin.endpoint.send(&request).await?;

    if REFUSED.contains(&response.status) {
        return Ok(None);
    }

    let signed: SignedIn = jellyfin
        .endpoint
        .decode(&response, "the sign-in was not accepted")?;

    Ok(Some(Signed {
        id: signed.user.id,
        token: signed.token,
    })
    .filter(|signed| !signed.id.is_empty() && a_key(&signed.token)))
}

async fn claim(
    jellyfin: &Jellyfin,
    name: &str,
    password: &str,
    device: &str,
) -> Result<bool, Failure> {
    let unclaimed = String::new();
    let Some(signed) = whoever(jellyfin, name, &unclaimed, device)
        .await?
        .filter(|signed| an_id(&signed.id))
    else {
        return Ok(false);
    };
    let body = serde_json::json!({ "CurrentPw": unclaimed, "NewPw": password }).to_string();
    let changing = super::carried(
        &jellyfin.request(
            Method::Post,
            &format!("/Users/{}/Password", signed.id),
            Some(body),
        ),
        &signed.token,
    );
    let response = jellyfin.endpoint.send(&changing).await?;
    jellyfin.endpoint.expect_success(&response)?;
    let leaving = super::carried(
        &jellyfin.request(Method::Post, "/Sessions/Logout", None),
        &signed.token,
    );
    let _ = jellyfin.endpoint.send(&leaving).await;
    Ok(true)
}

const REFUSED: [u16; 2] = [401, 403];

#[derive(serde::Deserialize)]
struct SignedIn {
    #[serde(rename = "User", default)]
    user: SignedInUser,
    #[serde(rename = "AccessToken", default)]
    token: String,
}

#[derive(serde::Deserialize, Default)]
struct SignedInUser {
    #[serde(rename = "Id", default)]
    id: String,
}

pub(super) async fn holdings(
    jellyfin: &Jellyfin,
    member: Option<&str>,
    most: u32,
) -> Result<Vec<Item>, Failure> {
    shelf::holdings(jellyfin, member, most).await
}

pub(super) async fn now_playing(
    jellyfin: &Jellyfin,
    member: Option<&str>,
) -> Result<Vec<Playback>, Failure> {
    playing::playing(jellyfin, member).await
}
