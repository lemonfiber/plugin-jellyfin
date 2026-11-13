use async_trait::async_trait;

use super::declared::{LIVE_STREAM_PARAMETER, SOURCE_PARAMETER};
use super::Jellyfin;
use crate::endpoint::form_encoded;
use crate::shape::{a_live_stream, hex, Id};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Channel, Failure, Programme, Tuned, Tuning};

const ABSENT: u16 = 404;

const BUFFERED: &str = "/LiveTv/LiveStreamFiles/";

const BUFFER_FILE: &str = "/stream.";

const EXTENSION_LONGEST: usize = 8;

const TRANSCODED: &str = "VideoCodec=h264&AudioCodec=aac&SegmentContainer=ts";

#[derive(serde::Deserialize)]
struct AccountResource {
    #[serde(rename = "Policy", default)]
    policy: PolicyResource,
}

#[derive(serde::Deserialize, Default)]
struct PolicyResource {
    #[serde(rename = "EnableLiveTvAccess", default)]
    live_tv: bool,
    #[serde(rename = "EnableMediaPlayback", default)]
    playback: bool,
    #[serde(rename = "IsDisabled", default)]
    disabled: bool,
}

#[derive(serde::Deserialize)]
struct ChannelsResource {
    #[serde(rename = "Items", default)]
    items: Vec<ChannelResource>,
}

#[derive(serde::Deserialize)]
struct ChannelResource {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Number", default)]
    number: Option<String>,
}

#[derive(serde::Deserialize)]
struct ProgrammesResource {
    #[serde(rename = "Items", default)]
    items: Vec<ProgrammeResource>,
}

#[derive(serde::Deserialize)]
struct ProgrammeResource {
    #[serde(rename = "ChannelId", default)]
    channel: String,
    #[serde(rename = "Name", default)]
    title: String,
    #[serde(rename = "OfficialRating", default)]
    certificate: Option<String>,
    #[serde(rename = "EndDate", default)]
    ends: Option<String>,
}

#[derive(serde::Serialize)]
struct Opening<'a> {
    #[serde(rename = "UserId")]
    member: &'a str,
    #[serde(rename = "AutoOpenLiveStream")]
    open: bool,
}

#[derive(serde::Deserialize)]
struct OpenedResource {
    #[serde(rename = "MediaSources", default)]
    sources: Vec<SourceResource>,
}

#[derive(serde::Deserialize)]
struct SourceResource {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Path", default)]
    path: Option<String>,
    #[serde(rename = "LiveStreamId", default)]
    live_stream: Option<String>,
}

#[async_trait]
impl Tuning for Jellyfin {
    async fn channels(&self, member: &str) -> Result<Vec<Channel>, Failure> {
        let member = self.account(member)?;
        if !self.may_watch_live_tv(member).await? {
            return Ok(Vec::new());
        }
        let watchable = self.watchable(member).await?;
        let mut airing = self.airing(&watchable).await?;
        Ok(watchable
            .into_iter()
            .map(|channel| Channel {
                now: airing
                    .iter()
                    .position(|programme| programme.channel == channel.id)
                    .map(|at| airing.swap_remove(at))
                    .map(|programme| Programme {
                        title: programme.title,
                        certificate: programme.certificate.filter(|rated| !rated.is_empty()),
                        ends: programme.ends,
                    }),
                id: channel.id,
                name: channel.name,
                number: channel.number.filter(|number| !number.is_empty()),
            })
            .collect())
    }

    async fn tune(&self, member: &str, channel: &str) -> Result<Option<Tuned>, Failure> {
        let member = self.account(member)?;
        let Some(asked) = Id::of(channel) else {
            return Ok(None);
        };
        if !self.may_watch_live_tv(member).await? {
            return Ok(None);
        }
        let Some(channel) = self
            .watchable(member)
            .await?
            .into_iter()
            .find(|watchable| same_id(&watchable.id, &asked.to_string()))
        else {
            return Ok(None);
        };
        self.opened(member, &channel.id).await.map(Some)
    }

    async fn untune(&self, closes_by: &str) -> Result<(), Failure> {
        if !a_live_stream(closes_by) {
            return Err(self.misshapen("a live stream"));
        }
        self.closed(closes_by).await
    }
}

impl Jellyfin {
    fn account<'a>(&self, member: &'a str) -> Result<Id<'a>, Failure> {
        Id::of(member).ok_or_else(|| self.misshapen("an account"))
    }

    async fn may_watch_live_tv(&self, member: Id<'_>) -> Result<bool, Failure> {
        let response = self
            .as_admin(Method::Get, &format!("/Users/{member}"), None)
            .await?;
        if response.status == ABSENT {
            return Ok(false);
        }
        let account: AccountResource = self.endpoint.decode(
            &response,
            "whether the account may watch live TV could not be read",
        )?;
        let policy = account.policy;
        Ok(policy.live_tv && policy.playback && !policy.disabled)
    }

    async fn watchable(&self, member: Id<'_>) -> Result<Vec<ChannelResource>, Failure> {
        let asked = form_encoded(&[
            ("userId", &member.to_string()),
            ("EnableUserData", "false"),
            ("EnableImages", "false"),
            ("AddCurrentProgram", "false"),
        ]);
        let response = self
            .as_admin(Method::Get, &format!("/LiveTv/Channels?{asked}"), None)
            .await?;
        let listed: ChannelsResource = self
            .endpoint
            .decode(&response, "the live TV channels could not be read")?;
        Ok(listed
            .items
            .into_iter()
            .filter(|channel| hex(&channel.id))
            .collect())
    }

    async fn airing(
        &self,
        channels: &[ChannelResource],
    ) -> Result<Vec<ProgrammeResource>, Failure> {
        if channels.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({
            "ChannelIds": channels.iter().map(|channel| channel.id.as_str()).collect::<Vec<_>>(),
            "IsAiring": true,
            "EnableUserData": false,
            "EnableImages": false,
            "EnableTotalRecordCount": false,
        })
        .to_string();
        let response = self
            .as_admin(Method::Post, "/LiveTv/Programs", Some(body))
            .await?;
        let listed: ProgrammesResource = self
            .endpoint
            .decode(&response, "what is on now could not be read")?;
        Ok(listed.items)
    }

    async fn opened(&self, member: Id<'_>, channel: &str) -> Result<Tuned, Failure> {
        let member = member.to_string();
        let body = serde_json::to_string(&Opening {
            member: &member,
            open: true,
        })
        .map_err(|why| {
            self.endpoint
                .refused(&format!("the channel could not be opened: {why}"))
        })?;
        let asked = form_encoded(&[("UserId", &member)]);
        let response = self
            .as_admin(
                Method::Post,
                &format!("/Items/{channel}/PlaybackInfo?{asked}"),
                Some(body),
            )
            .await?;
        let opened: OpenedResource = self
            .endpoint
            .decode(&response, "the channel's live stream could not be opened")?;
        let Some(source) = opened.sources.into_iter().next() else {
            return Err(self
                .endpoint
                .refused("Jellyfin opened no live stream for the channel"));
        };
        let Some(live_stream) = source.live_stream.filter(|id| !id.is_empty()) else {
            return Err(self
                .endpoint
                .refused("Jellyfin opened the channel without a live stream"));
        };
        if let Some(tuned) = tuned(channel, &source.id, &live_stream, source.path.as_deref()) {
            return Ok(tuned);
        }
        self.closed(&live_stream).await?;
        Err(self.endpoint.refused(
            "Jellyfin opened the channel without buffering it itself, so its stream is only at the tuner",
        ))
    }

    async fn closed(&self, live_stream: &str) -> Result<(), Failure> {
        let asked = form_encoded(&[("liveStreamId", live_stream)]);
        let response = self
            .as_admin(Method::Post, &format!("/LiveStreams/Close?{asked}"), None)
            .await?;
        self.endpoint.expect_success(&response)
    }
}

fn same_id(held: &str, asked: &str) -> bool {
    held.eq_ignore_ascii_case(&asked.replace('-', ""))
}

fn tuned(channel: &str, source: &str, live_stream: &str, path: Option<&str>) -> Option<Tuned> {
    if !(hex(channel) && hex(source) && a_live_stream(live_stream)) {
        return None;
    }
    let direct = buffered(path?)?;
    Some(Tuned {
        closes_by: live_stream.to_owned(),
        direct,
        transcoded: format!(
            "/Videos/{channel}/master.m3u8?{SOURCE_PARAMETER}={source}&{LIVE_STREAM_PARAMETER}={live_stream}&{TRANSCODED}"
        ),
    })
}

fn buffered(path: &str) -> Option<String> {
    let after_scheme = path.split_once("://").map_or(path, |(_, rest)| rest);
    let at = after_scheme.find('/')?;
    let (buffer, extension) = after_scheme
        .get(at..)?
        .strip_prefix(BUFFERED)?
        .split_once(BUFFER_FILE)?;
    let extension_shaped = (1..=EXTENSION_LONGEST).contains(&extension.len())
        && extension
            .bytes()
            .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit());
    (hex(buffer) && extension_shaped).then(|| format!("{BUFFERED}{buffer}{BUFFER_FILE}{extension}"))
}

#[cfg(test)]
mod tests;
