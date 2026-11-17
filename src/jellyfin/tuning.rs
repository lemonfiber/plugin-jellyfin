use async_trait::async_trait;

use super::declared::{LIVE_STREAM_PARAMETER, SOURCE_PARAMETER};
use super::Jellyfin;
use crate::endpoint::form_encoded;
use crate::shape::{a_live_stream, hex, Id};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::instant::HOUR;
use lemonfiber_ports::service::{
    Airing, Channel, Failure, Household as _, Listing, Programme, Tuned, Tuning, GUIDE_HOURS,
};
use lemonfiber_ports::Instant;
use std::collections::HashMap;

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
    #[serde(rename = "MaxParentalRating", default)]
    age_limit: Option<u32>,
    #[serde(rename = "BlockedTags", default)]
    blocked_tags: Vec<String>,
    #[serde(rename = "BlockUnratedItems", default)]
    unrated_blocked: Vec<String>,
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
    #[serde(rename = "Tags", default)]
    tags: Vec<String>,
    #[serde(
        rename = "StartDate",
        default,
        deserialize_with = "crate::jellyfin::moment"
    )]
    starts: Option<Instant>,
    #[serde(
        rename = "EndDate",
        default,
        deserialize_with = "crate::jellyfin::moment"
    )]
    ends: Option<Instant>,
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
        let Some(policy) = self.live_tv_policy(member).await? else {
            return Ok(Vec::new());
        };
        Ok(self
            .watched_now(member, &policy)
            .await?
            .into_iter()
            .map(|(channel, now)| Channel {
                now: now.map(|programme| Programme {
                    title: programme.title,
                    certificate: certified(programme.certificate),
                    ends: programme.ends,
                }),
                id: channel.id,
                name: channel.name,
                number: channel.number.filter(|number| !number.is_empty()),
            })
            .collect())
    }

    async fn guide(&self, member: &str, hours: u32) -> Result<Vec<Listing>, Failure> {
        let member = self.account(member)?;
        if !GUIDE_HOURS.contains(&hours) {
            return Err(self.endpoint.refused(&format!(
                "a guide is asked across {} to {} hours",
                GUIDE_HOURS.start(),
                GUIDE_HOURS.end()
            )));
        }
        let Some(policy) = self.live_tv_policy(member).await? else {
            return Ok(Vec::new());
        };
        let watchable: Vec<ChannelResource> = self
            .watched_now(member, &policy)
            .await?
            .into_iter()
            .map(|(channel, _)| channel)
            .collect();
        if watchable.is_empty() {
            return Ok(Vec::new());
        }
        let now = Instant::of(std::time::SystemTime::now());
        let (Some(from), Some(until)) = (stamped(now), stamped(now.later(u64::from(hours) * HOUR)))
        else {
            return Err(self
                .endpoint
                .refused("the guide's window falls outside the calendar"));
        };
        let held = aired(self.scheduled(&watchable, &from, &until, None).await?);
        let ages = self.ages(&policy).await?;
        let mut kept = aired(
            self.scheduled(&watchable, &from, &until, Some(member))
                .await?
                .into_iter()
                .filter(|programme| admitted(programme, &policy, &ages))
                .collect(),
        );
        Ok(watchable
            .into_iter()
            .map(|channel| Listing {
                until: held
                    .get(&channel.id)
                    .and_then(|programmes| programmes.iter().map(|airing| airing.ends).max()),
                programmes: kept.remove(&channel.id).unwrap_or_default(),
                channel: channel.id,
            })
            .collect())
    }

    async fn tune(&self, member: &str, channel: &str) -> Result<Option<Tuned>, Failure> {
        let member = self.account(member)?;
        let Some(asked) = Id::of(channel) else {
            return Ok(None);
        };
        let Some(policy) = self.live_tv_policy(member).await? else {
            return Ok(None);
        };
        let Some((channel, _)) = self
            .watched_now(member, &policy)
            .await?
            .into_iter()
            .find(|(watchable, _)| same_id(&watchable.id, &asked.to_string()))
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

    /// Each channel `member` may watch now, with the programme on it: one whose programme
    /// on now `policy` keeps from the account is left out, and where `policy` keeps anything
    /// from it, so is one with nothing known to be airing.
    async fn watched_now(
        &self,
        member: Id<'_>,
        policy: &PolicyResource,
    ) -> Result<Vec<(ChannelResource, Option<ProgrammeResource>)>, Failure> {
        let watchable = self.watchable(member).await?;
        if watchable.is_empty() {
            return Ok(Vec::new());
        }
        let ages = self.ages(policy).await?;
        let mut airing = self.airing(&watchable).await?;
        Ok(watchable
            .into_iter()
            .filter_map(|channel| {
                let now = airing
                    .iter()
                    .position(|programme| programme.channel == channel.id)
                    .map(|at| airing.swap_remove(at));
                let kept = match &now {
                    Some(programme) => admitted(programme, policy, &ages),
                    None => !restricted(policy),
                };
                kept.then_some((channel, now))
            })
            .collect())
    }

    async fn live_tv_policy(&self, member: Id<'_>) -> Result<Option<PolicyResource>, Failure> {
        let response = self
            .as_admin(Method::Get, &format!("/Users/{member}"), None)
            .await?;
        if response.status == ABSENT {
            return Ok(None);
        }
        let account: AccountResource = self.endpoint.decode(
            &response,
            "whether the account may watch live TV could not be read",
        )?;
        let policy = account.policy;
        Ok((policy.live_tv && policy.playback && !policy.disabled).then_some(policy))
    }

    async fn ages(&self, policy: &PolicyResource) -> Result<HashMap<String, u32>, Failure> {
        if policy.age_limit.is_none() {
            return Ok(HashMap::new());
        }
        Ok(self
            .ratings()
            .await?
            .into_iter()
            .map(|rating| (rating.name.to_lowercase(), rating.age))
            .collect())
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
        self.programmes(
            channels,
            vec![("IsAiring", true.into()), ("Fields", vec!["Tags"].into())],
            "what is on now could not be read",
        )
        .await
    }

    async fn scheduled(
        &self,
        channels: &[ChannelResource],
        from: &str,
        until: &str,
        member: Option<Id<'_>>,
    ) -> Result<Vec<ProgrammeResource>, Failure> {
        let mut asked = vec![
            ("MinEndDate", from.into()),
            ("MaxStartDate", until.into()),
            ("Fields", vec!["Tags"].into()),
        ];
        if let Some(member) = member {
            asked.push(("UserId", member.to_string().into()));
        }
        self.programmes(channels, asked, "the guide could not be read")
            .await
    }

    async fn programmes(
        &self,
        channels: &[ChannelResource],
        asked: Vec<(&str, serde_json::Value)>,
        unread: &str,
    ) -> Result<Vec<ProgrammeResource>, Failure> {
        let ids: Vec<&str> = channels.iter().map(|channel| channel.id.as_str()).collect();
        let body: serde_json::Map<String, serde_json::Value> = [
            ("ChannelIds", ids.into()),
            ("EnableUserData", false.into()),
            ("EnableImages", false.into()),
            ("EnableTotalRecordCount", false.into()),
        ]
        .into_iter()
        .chain(asked)
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
        let response = self
            .as_admin(
                Method::Post,
                "/LiveTv/Programs",
                Some(serde_json::Value::Object(body).to_string()),
            )
            .await?;
        let listed: ProgrammesResource = self.endpoint.decode(&response, unread)?;
        Ok(listed.items)
    }

    async fn opened(&self, member: Id<'_>, channel: &str) -> Result<Tuned, Failure> {
        let member = member.to_string();
        let body = serde_json::json!({
            "UserId": member,
            "AutoOpenLiveStream": true,
        })
        .to_string();
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

fn certified(certificate: Option<String>) -> Option<String> {
    certificate.filter(|rated| !rated.is_empty())
}

/// Whether `policy` keeps anything from the account: an age limit, a blocked tag, or
/// unrated items held back.
fn restricted(policy: &PolicyResource) -> bool {
    policy.age_limit.is_some()
        || !policy.blocked_tags.is_empty()
        || !policy.unrated_blocked.is_empty()
}

fn admitted(
    programme: &ProgrammeResource,
    policy: &PolicyResource,
    ages: &HashMap<String, u32>,
) -> bool {
    let blocked = programme.tags.iter().any(|tag| {
        policy
            .blocked_tags
            .iter()
            .any(|blocked| blocked.eq_ignore_ascii_case(tag))
    });
    let certificate = programme
        .certificate
        .as_deref()
        .filter(|rated| !rated.is_empty());
    let unrated_held = certificate.is_none() && !policy.unrated_blocked.is_empty();
    let too_old = policy.age_limit.is_some_and(|limit| {
        certificate.is_some_and(|rated| {
            ages.get(&rated.to_lowercase())
                .is_none_or(|age| *age > limit)
        })
    });
    !blocked && !unrated_held && !too_old
}

fn stamped(at: Instant) -> Option<String> {
    at.calendar().map(|text| format!("{text}Z"))
}

fn aired(scheduled: Vec<ProgrammeResource>) -> HashMap<String, Vec<Airing>> {
    let mut aired: HashMap<String, Vec<Airing>> = HashMap::new();
    for programme in scheduled {
        let (Some(starts), Some(ends)) = (programme.starts, programme.ends) else {
            continue;
        };
        aired.entry(programme.channel).or_default().push(Airing {
            title: programme.title,
            certificate: certified(programme.certificate),
            starts,
            ends,
        });
    }
    for programmes in aired.values_mut() {
        programmes.sort_by_key(|airing| airing.starts);
    }
    aired
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
