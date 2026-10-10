use async_trait::async_trait;

use super::item::EPISODE;
use super::{item_type, Jellyfin};
use crate::endpoint::form_encoded;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Failure, SeriesHeld, Upkeep};

const COUNTED: &str = "ChildCount,RecursiveItemCount";

const REFRESHED: [(&str, &str); 5] = [
    ("Recursive", "true"),
    ("MetadataRefreshMode", "FullRefresh"),
    ("ImageRefreshMode", "Default"),
    ("ReplaceAllMetadata", "false"),
    ("ReplaceAllImages", "false"),
];

#[derive(serde::Deserialize)]
struct SeriesPage {
    #[serde(rename = "Items", default)]
    items: Vec<Series>,
}

#[derive(serde::Deserialize)]
struct Series {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "ChildCount")]
    seasons: u32,
    #[serde(rename = "RecursiveItemCount", default)]
    episodes: Option<u32>,
}

#[derive(serde::Deserialize)]
struct Counted {
    #[serde(rename = "TotalRecordCount")]
    total: u32,
}

#[async_trait]
impl Upkeep for Jellyfin {
    async fn series_held(&self, most: u32) -> Result<Vec<SeriesHeld>, Failure> {
        held(self, most).await
    }

    async fn refresh(&self, id: &str) -> Result<(), Failure> {
        refreshed(self, id).await
    }
}

async fn held(jellyfin: &Jellyfin, most: u32) -> Result<Vec<SeriesHeld>, Failure> {
    let asked = form_encoded(&[
        ("Recursive", "true"),
        ("IncludeItemTypes", item_type(Kind::Tv)),
        ("Fields", COUNTED),
        ("SortBy", "SortName"),
        ("Limit", &most.to_string()),
    ]);
    let response = jellyfin
        .as_admin(Method::Get, &format!("/Items?{asked}"), None)
        .await?;
    let page: SeriesPage = jellyfin
        .endpoint
        .decode(&response, "the series the library holds could not be read")?;
    let mut held = Vec::with_capacity(page.items.len());
    for series in page.items {
        let episodes = match series.seasons {
            0 => filed_under(jellyfin, &series.id).await?,
            _ => series.episodes.unwrap_or_default(),
        };
        held.push(SeriesHeld {
            id: series.id,
            title: series.name,
            seasons: series.seasons,
            episodes,
        });
    }
    Ok(held)
}

async fn filed_under(jellyfin: &Jellyfin, id: &str) -> Result<u32, Failure> {
    let asked = form_encoded(&[
        ("ParentId", id),
        ("Recursive", "true"),
        ("IncludeItemTypes", EPISODE),
        ("IsMissing", "false"),
        ("Limit", "0"),
    ]);
    let response = jellyfin
        .as_admin(Method::Get, &format!("/Items?{asked}"), None)
        .await?;
    let counted: Counted = jellyfin.endpoint.decode(
        &response,
        "the episodes a series holds could not be counted",
    )?;
    Ok(counted.total)
}

async fn refreshed(jellyfin: &Jellyfin, id: &str) -> Result<(), Failure> {
    if !crate::shape::an_id(id) {
        return Err(jellyfin
            .endpoint
            .refused("that names nothing the media server holds"));
    }
    let asked = form_encoded(&REFRESHED);
    let response = jellyfin
        .as_admin(Method::Post, &format!("/Items/{id}/Refresh?{asked}"), None)
        .await?;
    jellyfin.endpoint.expect_success(&response)
}
