use regex::Regex;

use super::{reader, SIGNED_IN};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Channel, Declaring as _, Failure, LivePath, Programme, Tuned, Tuning as _,
};

const MEMBER: &str = "e79d47813bc24ccd8ca0385783e0208d";

const CHANNEL: &str = "adecd141257fda50dcb3439528c99432";

const LIVE_STREAM: &str = "e2329f4997b378e64ccf8fa396deb76e_af999c25a00715699361240d4c6c7a53_f928d7e0acd678b1a1f517973a4e3238";

const MAY_WATCH: &str = r#"{"Name":"member","Id":"e79d47813bc24ccd8ca0385783e0208d",
    "Policy":{"IsAdministrator":false,"IsDisabled":false,"EnableLiveTvManagement":false,
              "EnableLiveTvAccess":true,"EnableMediaPlayback":true}}"#;

const MAY_NOT_WATCH: &str = r#"{"Name":"member","Id":"e79d47813bc24ccd8ca0385783e0208d",
    "Policy":{"IsAdministrator":false,"IsDisabled":false,"EnableLiveTvManagement":false,
              "EnableLiveTvAccess":false,"EnableMediaPlayback":true}}"#;

const CHANNELS: &str = r#"{"Items":[{"Name":"Channel One","ServerId":"e9f8d01518fe48a3b56c55ae0ed8535e",
    "Id":"adecd141257fda50dcb3439528c99432","ChannelId":null,"Number":"7","ChannelNumber":"7",
    "IsFolder":false,"Type":"TvChannel","LocationType":"Remote","MediaType":"Video",
    "ChannelType":"TV"}],"TotalRecordCount":1,"StartIndex":0}"#;

const NO_CHANNELS: &str = r#"{"Items":[],"TotalRecordCount":0,"StartIndex":0}"#;

const AIRING: &str = r#"{"Items":[{"Name":"Hour 1","ServerId":"e9f8d01518fe48a3b56c55ae0ed8535e",
    "Id":"815d8704471b9df314f3dfd0cd495c11","OfficialRating":"TV-MA",
    "ChannelId":"adecd141257fda50dcb3439528c99432","RunTimeTicks":36000000000,"Type":"Program",
    "MediaType":"Unknown","EndDate":"2026-11-13T18:00:00.0000000Z",
    "StartDate":"2026-11-13T17:00:00.0000000Z"}],"TotalRecordCount":1,"StartIndex":0}"#;

const OPENED: &str = r#"{"MediaSources":[{"Protocol":"Http","Id":"f928d7e0acd678b1a1f517973a4e3238",
    "Path":"http://192.168.107.3:8096/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.ts",
    "Type":"Default","Container":"ts","IsRemote":false,"SupportsTranscoding":true,
    "SupportsDirectStream":true,"IsInfiniteStream":true,"RequiresOpening":true,"RequiresClosing":true,
    "LiveStreamId":"e2329f4997b378e64ccf8fa396deb76e_af999c25a00715699361240d4c6c7a53_f928d7e0acd678b1a1f517973a4e3238"}],
    "PlaySessionId":"53f2e0e962cd4b24981599e70addaa99"}"#;

const OPENED_AT_THE_TUNER: &str = r#"{"MediaSources":[{"Protocol":"Http","Id":"f928d7e0acd678b1a1f517973a4e3238",
    "Path":"http://files/channel.mp4","Container":"mp4","IsInfiniteStream":true,
    "LiveStreamId":"e2329f4997b378e64ccf8fa396deb76e_af999c25a00715699361240d4c6c7a53_f928d7e0acd678b1a1f517973a4e3238"}],
    "PlaySessionId":"53f2e0e962cd4b24981599e70addaa99"}"#;

const OPENED_NOTHING: &str =
    r#"{"MediaSources":[],"PlaySessionId":"53f2e0e962cd4b24981599e70addaa99"}"#;

const OPENED_WITHOUT_A_LIVE_STREAM: &str = r#"{"MediaSources":[{"Protocol":"Http","Id":"f928d7e0acd678b1a1f517973a4e3238",
    "Path":"http://192.168.107.3:8096/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.ts",
    "Container":"ts","IsInfiniteStream":true,"LiveStreamId":""}],
    "PlaySessionId":"53f2e0e962cd4b24981599e70addaa99"}"#;

fn jellyfin(
    account: &'static str,
    channels: &'static str,
    opened: &'static str,
) -> std::sync::Arc<Fake> {
    Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (
            Method::Get,
            "/Users/e79d47813bc24ccd8ca0385783e0208d",
            Answer::reply(200, account),
        ),
        (
            Method::Get,
            "/LiveTv/Channels",
            Answer::reply(200, channels),
        ),
        (Method::Post, "/LiveTv/Programs", Answer::reply(200, AIRING)),
        (Method::Post, "/PlaybackInfo", Answer::reply(200, opened)),
        (Method::Post, "/LiveStreams/Close", Answer::reply(204, "")),
    ])
}

fn sent(fake: &Fake, fragment: &str) -> Vec<lemonfiber_ports::http::Request> {
    fake.requests()
        .into_iter()
        .filter(|request| request.url.contains(fragment))
        .collect()
}

fn channel_one() -> Channel {
    Channel {
        id: CHANNEL.to_owned(),
        name: "Channel One".to_owned(),
        number: Some("7".to_owned()),
        now: Some(Programme {
            title: "Hour 1".to_owned(),
            certificate: Some("TV-MA".to_owned()),
            ends: Some("2026-11-13T18:00:00.0000000Z".to_owned()),
        }),
    }
}

#[tokio::test]
async fn an_account_live_tv_is_off_for_is_answered_no_channel_and_tuned_to_nothing() {
    let fake = jellyfin(MAY_NOT_WATCH, CHANNELS, OPENED);
    let jellyfin = reader(&fake);
    assert_eq!(jellyfin.channels(MEMBER).await.ok(), Some(Vec::new()));
    assert_eq!(jellyfin.tune(MEMBER, CHANNEL).await.ok(), Some(None));
    assert!(
        !fake.asked_for("/LiveTv/Channels") && !fake.asked_for("/PlaybackInfo"),
        "live TV was read or opened for an account it is off for"
    );
}

#[tokio::test]
async fn an_account_jellyfin_does_not_hold_is_answered_no_channel() {
    let fake = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (Method::Get, "/Users/", Answer::reply(404, "User not found")),
    ]);
    assert_eq!(reader(&fake).channels(MEMBER).await.ok(), Some(Vec::new()));
}

#[tokio::test]
async fn each_channel_is_answered_with_the_programme_on_it_now_and_its_certificate() {
    let fake = jellyfin(MAY_WATCH, CHANNELS, OPENED);
    assert_eq!(
        reader(&fake).channels(MEMBER).await.ok(),
        Some(vec![channel_one()])
    );
    let listed = sent(&fake, "/LiveTv/Channels");
    assert!(
        listed
            .iter()
            .all(|request| request.url.contains(&format!("userId={MEMBER}"))),
        "the channels were not read as the member's account sees them: {listed:?}"
    );
    let airing = sent(&fake, "/LiveTv/Programs");
    let body: serde_json::Value = airing
        .first()
        .and_then(|request| request.body.as_deref())
        .and_then(|body| serde_json::from_str(body).ok())
        .unwrap_or_default();
    assert_eq!(body.get("ChannelIds"), Some(&serde_json::json!([CHANNEL])));
    assert_eq!(body.get("IsAiring"), Some(&serde_json::json!(true)));
    assert!(
        body.get("UserId").is_none(),
        "the programme now was read through the member's limits, which hide it from the core: {body}"
    );
}

#[tokio::test]
async fn an_account_that_sees_no_channel_asks_no_guide() {
    let fake = jellyfin(MAY_WATCH, NO_CHANNELS, OPENED);
    assert_eq!(reader(&fake).channels(MEMBER).await.ok(), Some(Vec::new()));
    assert!(!fake.asked_for("/LiveTv/Programs"));
}

#[tokio::test]
async fn a_channel_the_account_does_not_see_is_tuned_to_nothing() {
    let fake = jellyfin(MAY_WATCH, NO_CHANNELS, OPENED);
    assert_eq!(reader(&fake).tune(MEMBER, CHANNEL).await.ok(), Some(None));
    assert!(!fake.asked_for("/PlaybackInfo"));
}

#[tokio::test]
async fn a_misshapen_channel_is_tuned_to_nothing_and_a_misshapen_member_is_refused() {
    let fake = jellyfin(MAY_WATCH, CHANNELS, OPENED);
    let jellyfin = reader(&fake);
    assert_eq!(jellyfin.tune(MEMBER, "../Users").await.ok(), Some(None));
    assert!(matches!(
        jellyfin.tune("../Users", CHANNEL).await,
        Err(Failure::Refused { .. })
    ));
    assert!(matches!(
        jellyfin.channels("e79d4781?x=1").await,
        Err(Failure::Refused { .. })
    ));
    assert!(!fake.asked_for("/PlaybackInfo"));
}

#[tokio::test]
async fn tuning_opens_the_live_stream_on_the_members_account() {
    let fake = jellyfin(MAY_WATCH, CHANNELS, OPENED);
    let tuned = reader(&fake).tune(MEMBER, CHANNEL).await;
    assert_eq!(
        tuned.ok().flatten().map(|tuned| tuned.closes_by),
        Some(LIVE_STREAM.to_owned())
    );
    let opened = sent(&fake, "/PlaybackInfo");
    let request = opened.first();
    assert!(
        request.is_some_and(|request| request
            .url
            .ends_with(&format!("/Items/{CHANNEL}/PlaybackInfo?UserId={MEMBER}"))),
        "the channel was not opened for the member: {opened:?}"
    );
    let body: serde_json::Value = request
        .and_then(|request| request.body.as_deref())
        .and_then(|body| serde_json::from_str(body).ok())
        .unwrap_or_default();
    assert_eq!(
        body,
        serde_json::json!({ "UserId": MEMBER, "AutoOpenLiveStream": true })
    );
}

fn covered(live: &[LivePath], path: &str) -> bool {
    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    live.iter().any(|declared| {
        let Ok(pattern) = Regex::new(&format!("^(?:{})", declared.pattern)) else {
            return false;
        };
        let Ok(shape) = Regex::new(&declared.stream) else {
            return false;
        };
        let Some(captured) = pattern.captures(route) else {
            return false;
        };
        match &declared.parameter {
            None => captured
                .name("stream")
                .is_some_and(|stream| shape.is_match(stream.as_str())),
            Some(parameter) => {
                let carried: Vec<(&str, &str)> = query
                    .split('&')
                    .filter_map(|pair| pair.split_once('='))
                    .filter(|(name, _)| name.eq_ignore_ascii_case(parameter))
                    .collect();
                matches!(carried.as_slice(), [(name, value)] if name == parameter && shape.is_match(value))
            }
        }
    })
}

#[tokio::test]
async fn both_paths_tuning_answers_are_covered_by_a_declared_live_path() {
    let fake = jellyfin(MAY_WATCH, CHANNELS, OPENED);
    let jellyfin = reader(&fake);
    let declared = jellyfin.declared().await.ok().map(|declared| declared.live);
    let tuned = jellyfin.tune(MEMBER, CHANNEL).await.ok().flatten();
    let live = declared.unwrap_or_default();
    let Tuned {
        direct, transcoded, ..
    } = tuned.unwrap_or(Tuned {
        closes_by: String::new(),
        direct: String::new(),
        transcoded: String::new(),
    });
    assert_eq!(
        direct,
        "/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.ts"
    );
    assert!(transcoded.starts_with(&format!("/Videos/{CHANNEL}/master.m3u8?")));
    assert!(transcoded.contains(&format!("LiveStreamId={LIVE_STREAM}")));
    for path in [&direct, &transcoded] {
        assert!(covered(&live, path), "{path} is covered by no live path");
    }
    let segment = "/Videos/adecd141257fda50dcb3439528c99432/hls/e748a320adfc77ead90b469cdd2ccbec/e748a320adfc77ead90b469cdd2ccbec0.ts";
    assert!(covered(
        &live,
        &format!("{segment}?LiveStreamId={LIVE_STREAM}")
    ));
    assert!(!covered(&live, segment));
    assert!(!covered(
        &live,
        &format!("/Videos/{CHANNEL}/stream?static=true&LiveStreamId={LIVE_STREAM}")
    ));
}

#[tokio::test]
async fn a_channel_jellyfin_does_not_buffer_itself_is_closed_and_refused() {
    let fake = jellyfin(MAY_WATCH, CHANNELS, OPENED_AT_THE_TUNER);
    assert!(matches!(
        reader(&fake).tune(MEMBER, CHANNEL).await,
        Err(Failure::Refused { .. })
    ));
    assert!(fake.asked_for(&format!("/LiveStreams/Close?liveStreamId={LIVE_STREAM}")));
}

#[tokio::test]
async fn an_account_whose_policy_cannot_be_read_is_refused() {
    let fake = jellyfin("<html>", CHANNELS, OPENED);
    assert!(matches!(
        reader(&fake).channels(MEMBER).await,
        Err(Failure::Refused { .. })
    ));
    assert!(!fake.asked_for("/LiveTv/Channels"));
}

#[tokio::test]
async fn a_channel_jellyfin_opens_no_live_stream_for_is_refused() {
    for opened in [OPENED_NOTHING, OPENED_WITHOUT_A_LIVE_STREAM] {
        let fake = jellyfin(MAY_WATCH, CHANNELS, opened);
        assert!(matches!(
            reader(&fake).tune(MEMBER, CHANNEL).await,
            Err(Failure::Refused { .. })
        ));
        assert!(!fake.asked_for("/LiveStreams/Close"));
    }
}

#[tokio::test]
async fn untuning_closes_the_live_stream_by_its_id() {
    let fake = jellyfin(MAY_WATCH, CHANNELS, OPENED);
    assert_eq!(reader(&fake).untune(LIVE_STREAM).await.ok(), Some(()));
    let closed = sent(&fake, "/LiveStreams/Close");
    assert!(
        closed
            .first()
            .is_some_and(|request| request.method == Method::Post
                && request
                    .url
                    .ends_with(&format!("/LiveStreams/Close?liveStreamId={LIVE_STREAM}"))),
        "the live stream was not closed: {closed:?}"
    );
}

#[tokio::test]
async fn a_misshapen_live_stream_is_refused_before_anything_is_closed() {
    let fake = jellyfin(MAY_WATCH, CHANNELS, OPENED);
    assert!(matches!(
        reader(&fake).untune("conformance-live-stream").await,
        Err(Failure::Refused { .. })
    ));
    assert!(fake.requests().is_empty());
}
