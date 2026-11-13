use std::sync::Arc;

use jellyfin_adapter::jellyfin::{Happenings, Jellyfin};
use lemonfiber_adapter_kit::recounting::{Connection, Entry, History};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Event, Failure, Happened, Recounting as _};
use serde_json::Value;

use super::{jellyfin, reader, SIGNED_IN};

const CONFIGURATION: &str = "/Plugins/71552a5a5c5c4350a2aeebe451a30173/Configuration";

const NEWEST_ITEM: &str =
    r#"{"Items":[{"Id":"a3","Name":"Heat","DateCreated":"2026-11-11T12:00:00.5Z"}]}"#;

const NEWEST_ENTRY: &str = r#"{"Items":[{"Id":42,"Name":"Library scan failed","Date":"2026-11-11T11:30:00Z","Severity":"Error"}]}"#;

const ITEMS: &str = r#"{"Items":[
    {"Id":"a3","Name":"Heat","DateCreated":"2026-11-11T12:00:00.5Z"},
    {"Id":"a2","Name":"Pilot","SeriesName":"Severance","DateCreated":"2026-11-11T11:00:00Z"},
    {"Id":"a0","Name":"Dune","DateCreated":"2026-11-11T10:00:00Z"}
]}"#;

const ENTRIES: &str = r#"{"Items":[
    {"Id":42,"Name":"Library scan failed","ShortOverview":"The disk is full","Date":"2026-11-11T11:30:00Z","Severity":"Error"},
    {"Id":41,"Name":"Server restarted","Date":"2026-11-11T10:30:00Z","Severity":"Information"},
    {"Id":40,"Name":"Older","Date":"2026-11-11T09:30:00Z","Severity":"Warning"}
]}"#;

const ACTIVE_WEBHOOK: &str =
    r#"[{"Id":"71552a5a-5c5c-4350-a2ae-ebe451a30173","Name":"Webhook","Status":"Active"}]"#;

const KEPT_DESTINATIONS: &str = r#"{"ServerUrl":"http://watch","DiscordOptions":[],"GenericOptions":[
    {"WebhookName":"lemonfiber","WebhookUri":"http://old/events/stale"},
    {"WebhookName":"theirs","WebhookUri":"http://theirs"}
]}"#;

fn heard(jellyfin: &Arc<Jellyfin>) -> Happenings {
    Happenings(Arc::downgrade(jellyfin))
}

fn read_from(fake: &Arc<Fake>) -> Arc<Jellyfin> {
    Arc::new(reader(fake))
}

#[tokio::test]
async fn the_newest_is_the_newest_item_s_added_time_and_id_and_the_newest_entry() {
    let fake = Fake::by_path(vec![
        ("/Users/AuthenticateByName", Answer::reply(200, SIGNED_IN)),
        ("/Items?", Answer::reply(200, NEWEST_ITEM)),
        (
            "/System/ActivityLog/Entries",
            Answer::reply(200, NEWEST_ENTRY),
        ),
    ]);
    let jellyfin = read_from(&fake);
    assert_eq!(
        heard(&jellyfin).newest().await.ok().as_deref(),
        Some("2026-11-11T12:00:00.5000000Z|a3|42")
    );
    assert!(fake.requests().iter().any(|request| {
        request.url.contains("IncludeItemTypes=Movie,Episode")
            && request
                .url
                .contains("SortBy=DateCreated&SortOrder=Descending")
    }));
    assert!(fake.asked_for("hasUserId=false"));
}

#[tokio::test]
async fn an_empty_jellyfin_s_newest_is_where_its_history_begins() {
    let fake = Fake::by_path(vec![
        ("/Users/AuthenticateByName", Answer::reply(200, SIGNED_IN)),
        ("/Items?", Answer::reply(200, r#"{"Items":[]}"#)),
        (
            "/System/ActivityLog/Entries",
            Answer::reply(200, r#"{"Items":[]}"#),
        ),
    ]);
    let jellyfin = read_from(&fake);
    assert_eq!(heard(&jellyfin).newest().await.ok().as_deref(), Some("||0"));
}

fn added(reference: &str, name: &str) -> Event {
    Event {
        event: Happened::Added,
        reference: Some(reference.to_owned()),
        name: name.to_owned(),
        reason: None,
        cleared: false,
    }
}

#[tokio::test]
async fn what_followed_the_cursor_comes_oldest_first_each_with_where_it_stands() {
    let fake = Fake::by_path(vec![
        ("/Users/AuthenticateByName", Answer::reply(200, SIGNED_IN)),
        ("/Items?", Answer::reply(200, ITEMS)),
        ("/System/ActivityLog/Entries", Answer::reply(200, ENTRIES)),
    ]);
    let jellyfin = read_from(&fake);
    let after = heard(&jellyfin)
        .after("2026-11-11T10:00:00.0000000Z|a0|40")
        .await
        .ok()
        .flatten();
    let at = |added: &str, id: &str, entry: u64| format!("{added}|{id}|{entry}");
    let ten = "2026-11-11T10:00:00.0000000Z";
    let eleven = "2026-11-11T11:00:00.0000000Z";
    assert_eq!(
        after,
        Some(vec![
            Entry {
                position: at(ten, "a0", 41),
                event: None,
            },
            Entry {
                position: at(eleven, "a2", 41),
                event: Some(added("a2", "Severance: Pilot")),
            },
            Entry {
                position: at(eleven, "a2", 42),
                event: Some(Event {
                    event: Happened::Health,
                    reference: None,
                    name: "Library scan failed".to_owned(),
                    reason: Some("The disk is full".to_owned()),
                    cleared: false,
                }),
            },
            Entry {
                position: at("2026-11-11T12:00:00.5000000Z", "a3", 42),
                event: Some(added("a3", "Heat")),
            },
        ])
    );
}

#[tokio::test]
async fn a_cursor_jellyfin_no_longer_holds_or_cannot_be_read_is_not_held() {
    let fake = Fake::by_path(vec![
        ("/Users/AuthenticateByName", Answer::reply(200, SIGNED_IN)),
        ("/Items?", Answer::reply(200, ITEMS)),
        ("/System/ActivityLog/Entries", Answer::reply(200, ENTRIES)),
    ]);
    let jellyfin = read_from(&fake);
    let heard = heard(&jellyfin);
    assert_eq!(heard.after("||50").await.ok(), Some(None));
    assert_eq!(heard.after("not a cursor").await.ok(), Some(None));
    assert_eq!(heard.after("a|b|c").await.ok(), Some(None));
    assert_eq!(heard.after("a|b|1|2").await.ok(), Some(None));
}

#[tokio::test]
async fn a_jellyfin_the_adapter_no_longer_holds_is_not_answering() {
    let fake = Fake::silent();
    let heard = heard(&read_from(&fake));
    assert!(matches!(
        heard.newest().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn without_the_webhook_plugin_running_jellyfin_cannot_push_and_nothing_is_changed() {
    for plugins in [
        "[]",
        r#"[{"Id":"71552a5a5c5c4350a2aeebe451a30173","Status":"Disabled"}]"#,
    ] {
        let fake = Fake::by_path(vec![
            ("/Users/AuthenticateByName", Answer::reply(200, SIGNED_IN)),
            ("/Plugins", Answer::reply(200, plugins)),
        ]);
        let jellyfin = read_from(&fake);
        let heard = heard(&jellyfin);
        assert_eq!(
            heard.connected("http://adapter/events/s").await.ok(),
            Some(false)
        );
        assert!(heard.disconnected().await.is_ok());
        assert!(!fake.asked_for("/Configuration"));
    }
}

fn posted_destinations(fake: &Fake) -> Vec<Value> {
    fake.requests()
        .iter()
        .filter(|request| request.method == Method::Post && request.url.ends_with(CONFIGURATION))
        .filter_map(|request| request.body.as_deref())
        .filter_map(|body| serde_json::from_str::<Value>(body).ok())
        .inspect(|posted| assert_eq!(posted["ServerUrl"], "http://watch"))
        .filter_map(|posted| {
            posted
                .get("GenericOptions")
                .and_then(Value::as_array)
                .cloned()
        })
        .flatten()
        .collect()
}

fn webhook_fake() -> Arc<Fake> {
    Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (
            Method::Get,
            CONFIGURATION,
            Answer::reply(200, KEPT_DESTINATIONS),
        ),
        (Method::Post, CONFIGURATION, Answer::reply(204, "")),
        (Method::Get, "/Plugins", Answer::reply(200, ACTIVE_WEBHOOK)),
    ])
}

#[tokio::test]
async fn with_the_webhook_plugin_running_its_one_destination_points_at_the_adapter_alone() {
    let fake = webhook_fake();
    let jellyfin = read_from(&fake);
    let at = "http://jellyfin-adapter:8080/events/secret";
    assert_eq!(heard(&jellyfin).connected(at).await.ok(), Some(true));
    let destinations = posted_destinations(&fake);
    let [theirs, ours] = destinations.as_slice() else {
        return assert_eq!(destinations.len(), 2);
    };
    assert_eq!(theirs["WebhookName"], "theirs");
    assert_eq!(ours["WebhookName"], "lemonfiber");
    assert_eq!(ours["WebhookUri"], at);
    assert_eq!(ours["NotificationTypes"], serde_json::json!(["ItemAdded"]));
    assert_eq!(ours["EnableMovies"], true);
    assert_eq!(ours["EnableEpisodes"], true);
    assert_eq!(ours["EnableWebhook"], true);
    assert_eq!(ours["SendAllProperties"], false);
}

#[tokio::test]
async fn a_stop_removes_the_adapter_s_destination_and_leaves_the_others() {
    let fake = webhook_fake();
    let jellyfin = read_from(&fake);
    assert!(heard(&jellyfin).disconnected().await.is_ok());
    let destinations = posted_destinations(&fake);
    let [theirs] = destinations.as_slice() else {
        return assert_eq!(destinations.len(), 1);
    };
    assert_eq!(theirs["WebhookName"], "theirs");
}

#[tokio::test]
async fn asked_with_nowhere_to_keep_a_cursor_it_refuses_and_asks_jellyfin_nothing() {
    let fake = Fake::silent();
    let said = jellyfin(&fake).events(None, None, None, None).await;
    assert!(
        matches!(&said, Err(Failure::Refused { service, .. }) if service == "jellyfin"),
        "{said:?}"
    );
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn the_recounting_kit_answers_events_through_its_recounter() {
    let dir = Scratch::new("jellyfin-recounting-kit");
    let fake = Fake::silent();
    let jellyfin = read_from(&fake);
    let _kit = jellyfin_adapter::adapter::recounting_kit(
        "key".to_owned(),
        Arc::clone(&jellyfin),
        dir.path(),
        "http://jellyfin-adapter:8080",
        Arc::new(lemonfiber_ports::narration::Silent),
    );
    assert_eq!(
        jellyfin.events(None, None, None, None).await.ok(),
        Some(None)
    );
    let partial = jellyfin
        .events(
            Some("https://core.invalid/api/notify/events"),
            None,
            None,
            None,
        )
        .await;
    assert!(
        matches!(&partial, Err(Failure::Disallowed { service, .. }) if service == "jellyfin"),
        "{partial:?}"
    );
}

fn page_of(entries: impl Iterator<Item = String>) -> String {
    format!(r#"{{"Items":[{}]}}"#, entries.collect::<Vec<_>>().join(","))
}

fn item_at(number: u32) -> String {
    format!(
        r#"{{"Id":"i{number}","Name":"Title {number}","DateCreated":"2026-11-12T{:02}:{:02}:00Z"}}"#,
        number / 60,
        number % 60
    )
}

fn entry_at(id: u32) -> String {
    format!(
        r#"{{"Id":{id},"Name":"Signed in","Date":"2026-11-11T11:00:00Z","Severity":"Information"}}"#
    )
}

#[tokio::test]
async fn a_full_page_of_newer_items_and_entries_reads_on_to_the_next() {
    let cursor_item = r#"{"Id":"a0","Name":"Dune","DateCreated":"2026-11-11T10:00:00Z"}"#;
    let fake = Fake::by_path(vec![
        ("/Users/AuthenticateByName", Answer::reply(200, SIGNED_IN)),
        (
            "StartIndex=0&",
            Answer::reply(200, page_of((2..=101).rev().map(item_at))),
        ),
        (
            "StartIndex=100&",
            Answer::reply(
                200,
                page_of([item_at(1), cursor_item.to_owned()].into_iter()),
            ),
        ),
        (
            "startIndex=0&limit=1&",
            Answer::reply(200, page_of(std::iter::once(entry_at(300)))),
        ),
        (
            "startIndex=0&",
            Answer::reply(200, page_of((201..=300).rev().map(entry_at))),
        ),
        (
            "startIndex=100&",
            Answer::reply(200, page_of([200, 199].into_iter().map(entry_at))),
        ),
    ]);
    let jellyfin = read_from(&fake);
    let after = heard(&jellyfin)
        .after("2026-11-11T10:00:00.0000000Z|a0|199")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let told: Vec<Option<String>> = after
        .iter()
        .filter_map(|entry| entry.event.as_ref())
        .map(|event| event.reference.clone())
        .collect();
    let added: Vec<Option<String>> = (1..=101).map(|number| Some(format!("i{number}"))).collect();
    assert_eq!(told, added);
    assert_eq!(after.len(), 202);
    assert_eq!(
        after.last().map(|entry| entry.position.as_str()),
        Some("2026-11-12T01:41:00.0000000Z|i101|300")
    );
}

#[tokio::test]
async fn a_webhook_configuration_that_is_not_an_object_is_refused_and_left_alone() {
    let fake = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (Method::Get, CONFIGURATION, Answer::reply(200, "[]")),
        (Method::Get, "/Plugins", Answer::reply(200, ACTIVE_WEBHOOK)),
    ]);
    let jellyfin = read_from(&fake);
    let said = heard(&jellyfin).connected("http://adapter/events/s").await;
    assert!(
        matches!(&said, Err(Failure::Refused { service, .. }) if service == "jellyfin"),
        "{said:?}"
    );
    assert!(!fake
        .requests()
        .iter()
        .any(|request| request.method == Method::Post && request.url.ends_with(CONFIGURATION)));
}

#[tokio::test]
async fn an_entry_naming_an_account_or_a_title_moves_the_cursor_and_tells_nothing() {
    let entries = r#"{"Items":[
        {"Id":46,"Name":"Subtitles failed to download from Open for Heat","Date":"2026-11-11T11:05:00Z","Severity":"Error","Type":"SubtitleDownloadFailure"},
        {"Id":45,"Name":"anna is locked out","Date":"2026-11-11T11:04:00Z","Severity":"Error","Type":"UserLockedOut"},
        {"Id":44,"Name":"Scheduled task Scan Media Library failed","Date":"2026-11-11T11:03:00Z","Severity":"Error","Type":"TaskFailed"},
        {"Id":43,"Name":"Failed login try from anna","Date":"2026-11-11T11:02:00Z","Severity":"Error","Type":"AuthenticationFailed"},
        {"Id":42,"Name":"Library scan failed","Date":"2026-11-11T11:30:00Z","Severity":"Error"}
    ]}"#;
    let fake = Fake::by_path(vec![
        ("/Users/AuthenticateByName", Answer::reply(200, SIGNED_IN)),
        ("/Items?", Answer::reply(200, NEWEST_ITEM)),
        ("/System/ActivityLog/Entries", Answer::reply(200, entries)),
    ]);
    let jellyfin = read_from(&fake);
    let after = heard(&jellyfin)
        .after("2026-11-11T12:00:00.5000000Z|a3|42")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let at = |entry: u64| format!("2026-11-11T12:00:00.5000000Z|a3|{entry}");
    let told: Vec<(String, Option<String>)> = after
        .iter()
        .map(|entry| {
            (
                entry.position.clone(),
                entry.event.as_ref().map(|event| event.name.clone()),
            )
        })
        .collect();
    assert_eq!(
        told,
        vec![
            (at(43), None),
            (
                at(44),
                Some("Scheduled task Scan Media Library failed".to_owned())
            ),
            (at(45), None),
            (at(46), None),
        ]
    );
    assert!(after
        .iter()
        .filter_map(|entry| entry.event.as_ref())
        .all(|event| event.event == Happened::Health));
}
