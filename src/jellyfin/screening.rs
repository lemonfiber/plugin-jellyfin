use async_trait::async_trait;

use super::item::ItemResource;
use super::Jellyfin;
use crate::endpoint::form_encoded;
use crate::shape::{a_device, a_picture, an_id, Id};
use lemonfiber_ports::http::{Method, Request};
use lemonfiber_ports::service::{
    EpisodeDetail, Failure, HowFar, Image, Item, ItemDetail, ItemProgress, Picture, Playback,
    Screening, SeasonDetail, PICTURE_MOST, PLAYER,
};

const TICKS: u64 = 10_000_000;

const ABSENT: u16 = 404;

const SIGNS_NONE_IN: [u16; 2] = [401, 403];

#[derive(serde::Deserialize)]
struct Pending {
    #[serde(rename = "Code", default)]
    code: String,
    #[serde(rename = "Secret", default)]
    secret: String,
}

#[derive(serde::Deserialize)]
struct Opened {
    #[serde(rename = "AccessToken", default)]
    token: String,
}

#[derive(serde::Deserialize)]
struct DetailResource {
    #[serde(flatten)]
    item: ItemResource,
    #[serde(rename = "Overview", default)]
    overview: Option<String>,
    #[serde(rename = "RunTimeTicks", default)]
    ticks: Option<u64>,
    #[serde(rename = "Genres", default)]
    genres: Vec<String>,
    #[serde(rename = "OfficialRating", default)]
    certificate: Option<String>,
    #[serde(rename = "PremiereDate", default)]
    premiered: Option<String>,
    #[serde(rename = "IndexNumber", default)]
    number: Option<u32>,
    #[serde(rename = "SeasonId", default)]
    season: Option<String>,
    #[serde(rename = "UserData", default)]
    watched: Option<UserData>,
}

#[derive(serde::Deserialize)]
struct UserData {
    #[serde(rename = "PlaybackPositionTicks", default)]
    position: u64,
}

#[derive(serde::Deserialize)]
struct DetailsResource {
    #[serde(rename = "Items", default)]
    items: Vec<DetailResource>,
}

#[derive(serde::Serialize)]
struct UserDataUpdate {
    #[serde(rename = "PlaybackPositionTicks")]
    position: u64,
    #[serde(rename = "Played")]
    played: bool,
}

#[async_trait]
impl Screening for Jellyfin {
    async fn holdings(&self, member: Option<&str>, most: u32) -> Result<Vec<Item>, Failure> {
        super::household::holdings(self, member, most).await
    }

    async fn playing(&self, member: Option<&str>) -> Result<Vec<Playback>, Failure> {
        super::household::now_playing(self, member).await
    }

    async fn signed_in(&self, member: &str, device: &str) -> Result<Option<String>, Failure> {
        opened(self, member, device).await
    }

    async fn title(&self, member: Option<&str>, id: &str) -> Result<Option<ItemDetail>, Failure> {
        titled(self, member, id).await
    }

    async fn part_way(&self, member: &str, most: u32) -> Result<Vec<ItemProgress>, Failure> {
        if !an_id(member) {
            return Err(self.misshapen("an account"));
        }
        let asked = form_encoded(&[
            ("userId", member),
            ("Limit", &most.to_string()),
            ("MediaTypes", "Video"),
        ]);
        let response = self
            .as_admin(Method::Get, &format!("/UserItems/Resume?{asked}"), None)
            .await?;
        let page: DetailsResource = self
            .endpoint
            .decode(&response, "what was part-way through could not be read")?;
        Ok(page
            .items
            .into_iter()
            .map(DetailResource::part_way)
            .collect())
    }

    async fn progressed(&self, member: &str, id: &str, how_far: &HowFar) -> Result<(), Failure> {
        progress(self, member, id, *how_far).await
    }

    async fn picture(
        &self,
        member: Option<&str>,
        id: &str,
        which: Picture,
    ) -> Result<Option<Image>, Failure> {
        pictured(self, member, id, which).await
    }

    async fn sign_out(&self, device: &str) -> Result<(), Failure> {
        if !a_device(device) {
            return Err(self.misshapen("a device"));
        }
        let asked = form_encoded(&[("id", device)]);
        let response = self
            .as_admin(Method::Delete, &format!("/Devices?{asked}"), None)
            .await?;
        self.endpoint.expect_success(&response)
    }
}

async fn opened(
    jellyfin: &Jellyfin,
    member: &str,
    device: &str,
) -> Result<Option<String>, Failure> {
    if !an_id(member) {
        return Err(jellyfin.misshapen("an account"));
    }
    if !a_device(device) {
        return Err(jellyfin.misshapen("a device"));
    }
    let response = jellyfin
        .endpoint
        .send(&as_device(jellyfin, "/QuickConnect/Initiate", None, device))
        .await?;
    if SIGNS_NONE_IN.contains(&response.status) {
        return Ok(None);
    }
    let pending: Pending = jellyfin.endpoint.decode(
        &response,
        "the media server would not issue the device a code",
    )?;
    let asked = form_encoded(&[("code", &pending.code), ("userId", member)]);
    let response = jellyfin
        .as_admin(
            Method::Post,
            &format!("/QuickConnect/Authorize?{asked}"),
            None,
        )
        .await?;
    let authorised: bool = jellyfin.endpoint.decode(
        &response,
        "the media server would not say whether it authorised the device",
    )?;
    if !authorised {
        return Err(jellyfin
            .endpoint
            .refused("the media server would not authorise the code it had just issued"));
    }
    let body = serde_json::json!({ "Secret": pending.secret }).to_string();
    let response = jellyfin
        .endpoint
        .send(&as_device(
            jellyfin,
            "/Users/AuthenticateWithQuickConnect",
            Some(body),
            device,
        ))
        .await?;
    let opened: Opened = jellyfin.endpoint.decode(
        &response,
        "the media server would not open the device's session",
    )?;
    if opened.token.is_empty() {
        return Err(jellyfin
            .endpoint
            .refused("the media server opened a session with no token"));
    }
    Ok(Some(opened.token))
}

fn pictured_at(id: Id<'_>, which: Picture) -> String {
    let named = match which {
        Picture::Poster => "Primary",
        Picture::Backdrop => "Backdrop",
    };
    format!("/Items/{id}/Images/{named}")
}

async fn pictured(
    jellyfin: &Jellyfin,
    member: Option<&str>,
    id: &str,
    which: Picture,
) -> Result<Option<Image>, Failure> {
    let member = account(jellyfin, member)?;
    let Some(id) = Id::of(id) else {
        return Ok(None);
    };
    if titled_as(jellyfin, member, id).await?.is_none() {
        return Ok(None);
    }
    let request = jellyfin.request(Method::Get, &pictured_at(id, which), None);
    let fetched = jellyfin.endpoint.fetch(&request, PICTURE_MOST).await?;
    if fetched.status == ABSENT {
        return Ok(None);
    }
    if !fetched.is_success() {
        return Err(jellyfin
            .endpoint
            .refused("the media server would not answer the picture"));
    }
    let Some(media_type) = fetched
        .header("content-type")
        .filter(|kind| a_picture(kind))
    else {
        return Ok(None);
    };
    let media_type = media_type.to_owned();
    Ok(fetched.bytes.map(|bytes| Image { media_type, bytes }))
}

fn as_device(jellyfin: &Jellyfin, path: &str, body: Option<String>, device: &str) -> Request {
    let mut request = jellyfin.request(Method::Post, path, body);
    request.headers.push((
        super::AUTHORIZATION_HEADER.to_owned(),
        format!(
            r#"MediaBrowser Client="{PLAYER}", Device="{PLAYER}", DeviceId="{device}", Version="1""#
        ),
    ));
    request
}

async fn titled(
    jellyfin: &Jellyfin,
    member: Option<&str>,
    id: &str,
) -> Result<Option<ItemDetail>, Failure> {
    let member = account(jellyfin, member)?;
    match Id::of(id) {
        Some(id) => titled_as(jellyfin, member, id).await,
        None => Ok(None),
    }
}

fn account<'a>(jellyfin: &Jellyfin, member: Option<&'a str>) -> Result<Option<Id<'a>>, Failure> {
    member
        .map(|member| Id::of(member).ok_or_else(|| jellyfin.misshapen("an account")))
        .transpose()
}

async fn titled_as(
    jellyfin: &Jellyfin,
    member: Option<Id<'_>>,
    id: Id<'_>,
) -> Result<Option<ItemDetail>, Failure> {
    let response = jellyfin
        .as_admin(Method::Get, &format!("/Items/{id}{}", whose(member)), None)
        .await?;
    if response.status == ABSENT {
        return Ok(None);
    }
    let detail: DetailResource = jellyfin
        .endpoint
        .decode(&response, "what the title is could not be read")?;
    let seasons = if detail.item.is_a_series() {
        seasons(jellyfin, member, id).await?
    } else {
        Vec::new()
    };
    Ok(Some(detail.titled(seasons)))
}

async fn progress(
    jellyfin: &Jellyfin,
    member: &str,
    id: &str,
    how_far: HowFar,
) -> Result<(), Failure> {
    if !an_id(member) {
        return Err(jellyfin.misshapen("an account"));
    }
    if !an_id(id) {
        return Err(jellyfin.misshapen("an item"));
    }
    let asked = form_encoded(&[("userId", member)]);
    let body = serde_json::to_string(&UserDataUpdate {
        position: if how_far.ended {
            0
        } else {
            how_far.position.saturating_mul(TICKS)
        },
        played: how_far.ended,
    })
    .unwrap_or_default();
    let response = jellyfin
        .as_admin(
            Method::Post,
            &format!("/UserItems/{id}/UserData?{asked}"),
            Some(body),
        )
        .await?;
    jellyfin.endpoint.expect_success(&response)
}

async fn seasons(
    jellyfin: &Jellyfin,
    member: Option<Id<'_>>,
    id: Id<'_>,
) -> Result<Vec<SeasonDetail>, Failure> {
    let response = jellyfin
        .as_admin(
            Method::Get,
            &format!("/Shows/{id}/Seasons{}", whose(member)),
            None,
        )
        .await?;
    let listed: DetailsResource = jellyfin
        .endpoint
        .decode(&response, "the series' seasons could not be read")?;
    let asked = match member {
        Some(member) => form_encoded(&[("userId", &member.to_string()), ("Fields", "Overview")]),
        None => form_encoded(&[("Fields", "Overview")]),
    };
    let response = jellyfin
        .as_admin(Method::Get, &format!("/Shows/{id}/Episodes?{asked}"), None)
        .await?;
    let episodes: DetailsResource = jellyfin
        .endpoint
        .decode(&response, "the series' episodes could not be read")?;
    Ok(grouped(listed.items, episodes.items))
}

fn whose(member: Option<Id<'_>>) -> String {
    member.map_or_else(String::new, |member| {
        format!("?{}", form_encoded(&[("userId", &member.to_string())]))
    })
}

fn grouped(seasons: Vec<DetailResource>, episodes: Vec<DetailResource>) -> Vec<SeasonDetail> {
    let mut episodes: Vec<(Option<String>, EpisodeDetail)> = episodes
        .into_iter()
        .map(|episode| (episode.season.clone(), episode.episode()))
        .collect();
    seasons
        .into_iter()
        .map(|season| {
            let id = season.item.id.clone();
            let (within, rest): (Vec<_>, Vec<_>) = episodes
                .drain(..)
                .partition(|(of, _)| of.as_deref() == Some(id.as_str()));
            episodes = rest;
            SeasonDetail {
                number: season.number,
                name: season.item.item().title,
                id,
                episodes: within.into_iter().map(|(_, episode)| episode).collect(),
            }
        })
        .collect()
}

fn minutes(ticks: Option<u64>) -> Option<u32> {
    ticks.and_then(|ticks| u32::try_from(ticks / TICKS / 60).ok())
}

impl DetailResource {
    fn titled(self, seasons: Vec<SeasonDetail>) -> ItemDetail {
        ItemDetail {
            overview: self.overview,
            minutes: minutes(self.ticks),
            genres: self.genres,
            certificate: self.certificate,
            released: self
                .premiered
                .map(|premiered| premiered.chars().take(10).collect()),
            seasons,
            item: self.item.item(),
        }
    }

    fn episode(self) -> EpisodeDetail {
        EpisodeDetail {
            number: self.number,
            overview: self.overview,
            minutes: minutes(self.ticks),
            item: self.item.item(),
        }
    }

    fn part_way(self) -> ItemProgress {
        ItemProgress {
            position: self.watched.map_or(0, |watched| watched.position / TICKS),
            length: self.ticks.map(|ticks| ticks / TICKS),
            item: self.item.item(),
        }
    }
}
