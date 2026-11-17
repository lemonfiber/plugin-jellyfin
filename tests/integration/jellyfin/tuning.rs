use regex::Regex;

use super::{reader, SIGNED_IN};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Airing, Channel, Declaring as _, Failure, Listing, LivePath, Programme, Tuned, Tuning as _,
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

const SCHEDULED: &str = r#"{"Items":[
    {"Name":"Hour 2","Id":"9a1f6e2c4b3d4e5f8a7b6c5d4e3f2a1b","OfficialRating":"",
     "ChannelId":"adecd141257fda50dcb3439528c99432","Type":"Program",
     "StartDate":"2026-11-13T18:00:00.0000000Z","EndDate":"2026-11-13T19:00:00.0000000Z"},
    {"Name":"Hour 1","Id":"815d8704471b9df314f3dfd0cd495c11","OfficialRating":"TV-MA",
     "ChannelId":"adecd141257fda50dcb3439528c99432","Type":"Program",
     "StartDate":"2026-11-13T17:00:00.0000000Z","EndDate":"2026-11-13T18:00:00.0000000Z"},
    {"Name":"Fright Night","Id":"5c4b3a2918f7e6d5c4b3a2918f7e6d5c","OfficialRating":"TV-PG",
     "ChannelId":"adecd141257fda50dcb3439528c99432","Type":"Program","Tags":["horror"],
     "StartDate":"2026-11-13T19:00:00.0000000Z","EndDate":"2026-11-13T20:30:00.0000000Z"},
    {"Name":"Undated","Id":"0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e","OfficialRating":"TV-G",
     "ChannelId":"adecd141257fda50dcb3439528c99432","Type":"Program",
     "EndDate":"2026-11-13T21:00:00.0000000Z"}],"TotalRecordCount":4,"StartIndex":0}"#;

const GUARDED: &str = r#"{"Name":"member","Id":"e79d47813bc24ccd8ca0385783e0208d",
    "Policy":{"IsAdministrator":false,"IsDisabled":false,"EnableLiveTvAccess":true,
              "EnableMediaPlayback":true,"MaxParentalRating":13,"BlockedTags":["Horror"],
              "BlockUnratedItems":["Movie"]}}"#;

const ADULT: &str = r#"{"Name":"member","Id":"e79d47813bc24ccd8ca0385783e0208d",
    "Policy":{"IsAdministrator":false,"IsDisabled":false,"EnableLiveTvAccess":true,
              "EnableMediaPlayback":true,"MaxParentalRating":18}}"#;

const RATINGS: &str = r#"[{"Name":"TV-PG","Value":10},{"Name":"TV-MA","Value":17}]"#;

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

fn guided(account: &'static str) -> std::sync::Arc<Fake> {
    guided_with(account, RATINGS)
}

fn guided_with(account: &'static str, ratings: &'static str) -> std::sync::Arc<Fake> {
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
            Answer::reply(200, CHANNELS),
        ),
        (
            Method::Post,
            "/LiveTv/Programs",
            Answer::reply(200, SCHEDULED),
        ),
        (
            Method::Get,
            "/Localization/ParentalRatings",
            Answer::reply(200, ratings),
        ),
    ])
}

fn asked_for_the_guide(fake: &Fake) -> Vec<serde_json::Value> {
    sent(fake, "/LiveTv/Programs")
        .iter()
        .filter_map(|request| request.body.as_deref())
        .filter_map(|body| serde_json::from_str(body).ok())
        .collect()
}

fn instant(stamp: &str) -> Option<lemonfiber_ports::Instant> {
    lemonfiber_ports::Instant::from_service_text(stamp)
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
            ends: lemonfiber_ports::Instant::from_service_text("2026-11-13T18:00:00.0000000Z"),
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

fn aired(title: &str, certificate: Option<&str>, starts: &str, ends: &str) -> Option<Airing> {
    Some(Airing {
        title: title.to_owned(),
        certificate: certificate.map(str::to_owned),
        starts: lemonfiber_ports::Instant::from_service_text(starts)?,
        ends: lemonfiber_ports::Instant::from_service_text(ends)?,
    })
}

#[tokio::test]
async fn each_channel_s_guide_holds_its_dated_programmes_in_the_order_they_air() {
    let fake = guided(MAY_WATCH);
    let programmes: Vec<Airing> = [
        aired(
            "Hour 1",
            Some("TV-MA"),
            "2026-11-13T17:00:00Z",
            "2026-11-13T18:00:00Z",
        ),
        aired(
            "Hour 2",
            None,
            "2026-11-13T18:00:00Z",
            "2026-11-13T19:00:00Z",
        ),
        aired(
            "Fright Night",
            Some("TV-PG"),
            "2026-11-13T19:00:00Z",
            "2026-11-13T20:30:00Z",
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert_eq!(programmes.len(), 3);
    assert_eq!(
        reader(&fake).guide(MEMBER, 12).await.ok(),
        Some(vec![Listing {
            channel: CHANNEL.to_owned(),
            programmes,
            until: instant("2026-11-13T20:30:00Z"),
        }])
    );
    assert!(!fake.asked_for("/Localization/ParentalRatings"));
    assert!(sent(&fake, "/LiveTv/Channels")
        .iter()
        .all(|request| request.url.contains(&format!("userId={MEMBER}"))));
}

#[tokio::test]
async fn the_guide_is_read_across_the_hours_asked_from_now_once_whole_and_once_as_the_member_may_watch_it(
) {
    let fake = guided(MAY_WATCH);
    let before = lemonfiber_ports::Instant::of(std::time::SystemTime::now());
    assert!(reader(&fake).guide(MEMBER, 30).await.is_ok());
    let after = lemonfiber_ports::Instant::of(std::time::SystemTime::now());
    let asked = asked_for_the_guide(&fake);
    assert_eq!(
        asked
            .iter()
            .map(|body| body.get("UserId").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![None, Some(MEMBER)]
    );
    assert!(asked
        .iter()
        .all(|body| body.get("Fields") == Some(&serde_json::json!(["Tags"]))));
    let body = asked.first().cloned().unwrap_or_default();
    let read = |field: &str| {
        body.get(field)
            .and_then(serde_json::Value::as_str)
            .and_then(lemonfiber_ports::Instant::from_service_text)
    };
    let from = read("MinEndDate");
    assert!(
        from.is_some_and(|from| before <= from && from <= after),
        "the guide was not read from now: {body}"
    );
    assert_eq!(
        read("MaxStartDate")
            .zip(from)
            .map(|(until, from)| until.since(from)),
        Some(30 * lemonfiber_ports::instant::HOUR)
    );
    assert_eq!(body.get("ChannelIds"), Some(&serde_json::json!([CHANNEL])));
    assert!(
        body.get("IsAiring").is_none(),
        "the guide was read only for now: {body}"
    );
}

#[tokio::test]
async fn a_programme_above_the_member_s_rating_carrying_a_blocked_tag_or_unrated_where_unrated_is_blocked_is_left_out_and_the_guide_still_ends_where_jellyfin_s_does(
) {
    let fake = guided(GUARDED);
    assert_eq!(
        reader(&fake).guide(MEMBER, 12).await.ok(),
        Some(vec![Listing {
            channel: CHANNEL.to_owned(),
            programmes: Vec::new(),
            until: instant("2026-11-13T20:30:00Z"),
        }])
    );
}

#[tokio::test]
async fn under_an_age_limit_a_programme_whose_certificate_jellyfin_s_ratings_do_not_name_is_left_out(
) {
    let fake = guided_with(ADULT, r#"[{"Name":"TV-MA","Value":17}]"#);
    let titles: Option<Vec<String>> = reader(&fake)
        .guide(MEMBER, 12)
        .await
        .ok()
        .and_then(|listings| listings.into_iter().next())
        .map(|listing| {
            listing
                .programmes
                .into_iter()
                .map(|airing| airing.title)
                .collect()
        });
    assert_eq!(titles, Some(vec!["Hour 1".to_owned(), "Hour 2".to_owned()]));
}

#[tokio::test]
async fn a_guide_whose_ratings_cannot_be_read_is_refused() {
    let fake = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (
            Method::Get,
            "/Users/e79d47813bc24ccd8ca0385783e0208d",
            Answer::reply(200, GUARDED),
        ),
        (
            Method::Get,
            "/LiveTv/Channels",
            Answer::reply(200, CHANNELS),
        ),
        (
            Method::Post,
            "/LiveTv/Programs",
            Answer::reply(200, SCHEDULED),
        ),
        (
            Method::Get,
            "/Localization/ParentalRatings",
            Answer::reply(200, "<html>"),
        ),
    ]);
    assert!(matches!(
        reader(&fake).guide(MEMBER, 12).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_account_that_sees_no_channel_reads_no_guide() {
    let fake = jellyfin(MAY_WATCH, NO_CHANNELS, OPENED);
    assert_eq!(reader(&fake).guide(MEMBER, 12).await.ok(), Some(Vec::new()));
    assert!(!fake.asked_for("/LiveTv/Programs"));
}

#[tokio::test]
async fn an_account_live_tv_is_off_for_is_answered_no_guide() {
    let fake = guided(MAY_NOT_WATCH);
    assert_eq!(reader(&fake).guide(MEMBER, 12).await.ok(), Some(Vec::new()));
    assert!(!fake.asked_for("/LiveTv/Channels") && !fake.asked_for("/LiveTv/Programs"));
}

#[tokio::test]
async fn a_guide_asked_outside_its_hours_or_for_a_misshapen_member_is_refused_before_anything_is_read(
) {
    let fake = guided(MAY_WATCH);
    let jellyfin = reader(&fake);
    for hours in [0, 169] {
        assert!(matches!(
            jellyfin.guide(MEMBER, hours).await,
            Err(Failure::Refused { .. })
        ));
    }
    assert!(matches!(
        jellyfin.guide("../Users", 12).await,
        Err(Failure::Refused { .. })
    ));
    assert!(fake.requests().is_empty());
}
