use lemonfiber_ports::service::Declared;

use super::declared;
use super::guard::{matching, passes, CLOSED, MEMBER, OPEN};

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    path: String,
    presents: String,
    passes: bool,
}

fn dashed(id: &str) -> String {
    [(0, 8), (8, 12), (12, 16), (16, 20), (20, 32)]
        .iter()
        .filter_map(|&(from, to)| id.get(from..to))
        .collect::<Vec<_>>()
        .join("-")
}

fn target(path: &str) -> String {
    path.replace("{open_dashed}", &dashed(OPEN))
        .replace("{closed_dashed}", &dashed(CLOSED))
        .replace("{closed_upper}", &CLOSED.to_ascii_uppercase())
        .replace("{token_upper}", &MEMBER.to_ascii_uppercase())
        .replace("{open}", OPEN)
        .replace("{closed}", CLOSED)
        .replace("{token}", MEMBER)
}

fn headers(presents: &str) -> Vec<(String, String)> {
    let unknown = "0".repeat(32);
    let header = match presents {
        "authorization" => Some((
            "Authorization",
            format!(
                r#"MediaBrowser Client="door", Device="door", DeviceId="door", Version="1", Token="{MEMBER}""#
            ),
        )),
        "x-emby-token" => Some(("X-Emby-Token", MEMBER.to_owned())),
        "x-mediabrowser-token" => Some(("X-MediaBrowser-Token", MEMBER.to_owned())),
        "unknown" => Some(("X-Emby-Token", unknown)),
        "bearer" => Some(("Authorization", format!("Bearer {MEMBER}"))),
        "bearer-unknown" => Some(("Authorization", format!("Bearer {unknown}"))),
        "bearer-smuggled" => Some((
            "Authorization",
            format!(r#"Bearer {MEMBER}", DeviceId="door"#),
        )),
        _ => None,
    };
    header
        .map(|(name, value)| vec![(name.to_owned(), value)])
        .unwrap_or_default()
}

fn differing(declared: &Declared, cases: &[Case]) -> Vec<String> {
    cases
        .iter()
        .filter(|case| {
            passes(declared, &target(&case.path), &headers(&case.presents)) != case.passes
        })
        .map(|case| case.name.clone())
        .collect()
}

#[tokio::test]
async fn a_guard_enforcing_the_declaration_answers_every_case_of_the_door_as_the_door_does() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("door.json")).unwrap_or_default();
    assert_eq!(cases.len(), 87);
    let declared = declared().await;
    assert_eq!(
        declared.map(|declared| differing(&declared, &cases)),
        Some(Vec::new())
    );
}

#[tokio::test]
async fn a_trickplay_tile_asks_about_the_source_it_names_under_every_spelling_jellyfin_reads() {
    let tile = "/Videos/{open}/Trickplay/320/0.jpg";
    let cases: Vec<Case> = [
        ("of-its-own-source", "?MediaSourceId={open}", true),
        ("naming-another-item", "?MediaSourceId={closed}", false),
        (
            "naming-another-item-in-another-case",
            "?mediasourceid={closed}",
            false,
        ),
        (
            "naming-another-item-in-capitals",
            "?MEDIASOURCEID={closed}",
            false,
        ),
        (
            "naming-its-own-source-in-another-case",
            "?mediasourceid={open}",
            false,
        ),
        (
            "naming-its-source-twice",
            "?MediaSourceId={open}&MediaSourceId={closed}",
            false,
        ),
        (
            "naming-the-same-source-twice",
            "?MediaSourceId={open}&MediaSourceId={open}",
            false,
        ),
        (
            "with-a-letter-of-the-name-encoded",
            "?MediaSource%49d={closed}",
            false,
        ),
        (
            "with-the-source-in-capitals",
            "?MediaSourceId={closed_upper}",
            false,
        ),
        ("naming-another-item-by-its-id", "?itemId={closed}", false),
        (
            "naming-another-item-by-its-id-in-capitals",
            "?ITEMID={closed}",
            false,
        ),
        ("naming-a-live-stream", "?LiveStreamId=planted", false),
        ("with-no-source", "", true),
    ]
    .into_iter()
    .map(|(name, query, passes)| Case {
        name: format!("trickplay-tile-{name}"),
        path: format!("{tile}{query}"),
        presents: "authorization".to_owned(),
        passes,
    })
    .collect();
    let declared = declared().await;
    assert_eq!(
        declared.map(|declared| differing(&declared, &cases)),
        Some(Vec::new())
    );
}

#[tokio::test]
async fn every_pattern_and_shape_the_declaration_holds_compiles() {
    let Some(declared) = declared().await else {
        unreachable!("jellyfin answers its declaration");
    };
    let mut patterns: Vec<&str> = declared.refused.iter().map(String::as_str).collect();
    for path in &declared.bytes {
        patterns.push(&path.pattern);
        patterns.push(&path.item);
        patterns.extend(path.exempt.iter().map(String::as_str));
    }
    patterns.push(&declared.token.shape);
    assert!(patterns
        .iter()
        .all(|pattern| regex::Regex::new(pattern).is_ok()));
    assert!(matching(&declared.token.shape, MEMBER));
}

#[tokio::test]
async fn a_byte_route_naming_an_id_with_more_before_or_after_it_is_refused() {
    let Some(declared) = declared().await else {
        unreachable!("jellyfin answers its declaration");
    };
    let session = headers("authorization");
    assert!(passes(
        &declared,
        &target("/Videos/{open}/stream"),
        &session
    ));
    for path in [
        "/Videos/x{open}/stream",
        "/Videos/{open}x/stream",
        "/Items/00{open}/Images/Primary",
        "/Videos/{open}/Trickplay/320/tiles.m3u8?MediaSourceId=x{open}",
        "/Videos/{open}/Trickplay/320/0.jpg?MediaSourceId={open}x",
    ] {
        assert!(!passes(&declared, &target(path), &session), "{path}");
    }
    for path in &declared.bytes {
        assert!(!matching(&path.item, &format!("x{OPEN}")));
        assert!(!matching(&path.item, &format!("{OPEN}x")));
    }
}
