use lemonfiber_fixtures::http::Fake;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Addressee, Declared, Declaring as _, Held, Presentation, Statuses,
};

use super::jellyfin;

mod door;
mod guard;

const PREFIXED: &str = "(?i)^(?:/emby|/mediabrowser)?";

async fn declared() -> Option<Declared> {
    jellyfin(&Fake::silent()).declared().await.ok()
}

#[tokio::test]
async fn jellyfin_is_not_asked_anything_to_answer_the_declaration() {
    let fake = Fake::silent();
    assert!(jellyfin(&fake).declared().await.is_ok());
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn the_declaration_is_one_a_guard_can_enforce() {
    let declared = declared().await;
    assert!(declared.is_some());
    assert_eq!(
        declared
            .clone()
            .and_then(|declared| declared.checked().ok()),
        declared
    );
}

#[tokio::test]
async fn every_byte_route_is_matched_at_the_root_and_under_both_prefixes_without_regard_to_case() {
    let bytes = declared().await.map(|declared| declared.bytes);
    let patterns: Option<Vec<String>> = bytes
        .clone()
        .map(|bytes| bytes.into_iter().map(|path| path.pattern).collect());
    assert_eq!(
        patterns,
        Some(vec![
            format!("{PREFIXED}/Videos/(?P<item>[^/]+)/Trickplay(?:/|$)"),
            format!("{PREFIXED}/(?:Videos|Audio|LiveTv/LiveStreamFiles|LiveTv/LiveRecordings)/(?P<item>[^/]+)(?:/|$)"),
            format!("{PREFIXED}/Items/(?P<item>[^/]+)/(?:Images|Download|File)(?:/|$)"),
        ])
    );
    for path in bytes.unwrap_or_default() {
        assert_eq!(
            path.item,
            "(?i)^[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}$"
        );
    }
}

#[tokio::test]
async fn only_the_two_routes_under_videos_that_name_no_item_pass_unasked() {
    let exempt: Option<Vec<Vec<String>>> = declared()
        .await
        .map(|declared| declared.bytes.into_iter().map(|path| path.exempt).collect());
    assert_eq!(
        exempt,
        Some(vec![
            Vec::new(),
            vec![format!(
                "{PREFIXED}/Videos/(?:ActiveEncodings|MergeVersions)$"
            )],
            Vec::new(),
        ])
    );
}

#[tokio::test]
async fn every_byte_route_refuses_another_item_and_every_trickplay_route_asks_about_its_source() {
    let naming: Option<Vec<Vec<(String, Held)>>> = declared().await.map(|declared| {
        declared
            .bytes
            .into_iter()
            .map(|path| {
                path.naming
                    .into_iter()
                    .map(|named| (named.parameter, named.held))
                    .collect()
            })
            .collect()
    });
    let refused = vec![
        ("itemId".to_owned(), Held::Refused),
        ("LiveStreamId".to_owned(), Held::Refused),
    ];
    let mut trickplay = refused.clone();
    trickplay.push((
        "MediaSourceId".to_owned(),
        Held::Asked {
            shape: "^[0-9a-f]{32}$".to_owned(),
        },
    ));
    assert_eq!(naming, Some(vec![trickplay, refused.clone(), refused]));
}

#[tokio::test]
async fn a_picture_found_by_a_name_and_the_splash_screen_are_refused_unasked() {
    assert_eq!(
        declared().await.map(|declared| declared.refused),
        Some(vec![
            format!(
                "{PREFIXED}/(?:Artists|Genres|MusicGenres|Persons|Studios)/[^/]+/Images(?:/|$)"
            ),
            format!("{PREFIXED}/Branding/Splashscreen/?$"),
        ])
    );
}

#[tokio::test]
async fn the_guard_asks_jellyfin_for_the_item_as_the_members_own_token_sees_it() {
    let question = declared().await.and_then(|declared| declared.question);
    assert_eq!(
        question
            .as_ref()
            .map(|asked| (asked.method, asked.path.as_str(), asked.to)),
        Some((Method::Get, "/Items/{item}", Addressee::Upstream))
    );
    let answers = question.map(|asked| asked.answers);
    assert_eq!(
        answers.map(|answers| (
            answers.may,
            answers.may_not,
            answers.no_session,
            answers.not_from_there
        )),
        Some((
            vec![Statuses { from: 200, to: 299 }],
            vec![Statuses::only(404)],
            vec![Statuses::only(401)],
            vec![Statuses::only(403)],
        ))
    );
}

#[tokio::test]
async fn a_bearer_token_of_jellyfins_shape_is_put_in_jellyfins_own_form() {
    let token = declared().await.map(|declared| declared.token);
    assert_eq!(
        token.as_ref().map(|token| &token.presented),
        Some(&Presentation::Header {
            name: "Authorization".to_owned(),
            value: r#"MediaBrowser Token="{token}""#.to_owned(),
        })
    );
    assert_eq!(
        token.map(|token| (token.shape, token.parameters)),
        Some((
            "^[0-9a-f]{32}$".to_owned(),
            vec!["ApiKey".to_owned(), "api_key".to_owned()]
        ))
    );
}

#[tokio::test]
async fn jellyfin_listens_on_its_own_port_and_its_own_encrypted_one() {
    assert_eq!(
        declared()
            .await
            .map(|declared| (declared.ports.upstream, declared.ports.encrypted)),
        Some((8096, Some(8920)))
    );
}

#[tokio::test]
async fn an_item_is_streamed_and_pictured_where_jellyfin_serves_it_beneath_the_door() {
    let locations = declared().await.map(|declared| declared.locations);
    assert_eq!(
        locations.map(|at| (at.stream, at.poster, at.backdrop)),
        Some((
            "{base}/Videos/{item}/master.m3u8?MediaSourceId={item}&VideoCodec=h264,hevc&AudioCodec=aac,mp3,ac3,eac3&SegmentContainer=mp4".to_owned(),
            "{base}/Items/{item}/Images/Primary".to_owned(),
            "{base}/Items/{item}/Images/Backdrop".to_owned(),
        ))
    );
}
