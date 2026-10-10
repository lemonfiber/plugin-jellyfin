use super::{reader, OURS, OURS_TOO, SIGNED_IN};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::Household;
use lemonfiber_ports::service::{Allowed, AppKeys as _, Failure};

#[tokio::test]
async fn only_the_keys_filed_under_the_name_asked_for_are_listed() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, OURS_TOO),
    ]);
    assert_eq!(
        reader(&fake).filed_as("lemonfiber").await.ok(),
        Some(vec![OURS.to_owned()])
    );
}

#[tokio::test]
async fn a_key_is_revoked_at_its_own_address() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(204, "")]);
    assert!(reader(&fake).revoke(OURS).await.is_ok());
    let deleted: Vec<String> = fake
        .requests()
        .into_iter()
        .filter(|asked| asked.method == Method::Delete)
        .map(|asked| asked.url)
        .collect();
    assert_eq!(deleted, [format!("http://127.0.0.1:8096/Auth/Keys/{OURS}")]);
}

#[tokio::test]
async fn a_key_shaped_like_a_path_is_refused_before_anything_is_asked() {
    let fake = Fake::silent();
    for key in ["../Users", "a?b=c", ""] {
        assert!(matches!(
            reader(&fake).revoke(key).await,
            Err(Failure::Refused { .. })
        ));
        assert!(matches!(
            reader(&fake).answers_to(key).await,
            Err(Failure::Refused { .. })
        ));
    }
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn a_revocation_the_server_refuses_is_reported() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(500, "")]);
    assert!(reader(&fake).revoke(OURS).await.is_err());
}

#[tokio::test]
async fn a_key_list_that_cannot_be_read_is_reported() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);
    assert!(reader(&fake).filed_as("lemonfiber").await.is_err());
}

#[tokio::test]
async fn a_key_is_minted_under_its_name_and_the_new_one_is_answered() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[]}"#),
        Answer::reply(204, ""),
        Answer::reply(200, OURS_TOO),
    ]);
    assert_eq!(
        reader(&fake).mint("lemonfiber").await.ok(),
        Some(OURS.to_owned())
    );
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[]}"#),
        Answer::reply(204, ""),
        Answer::reply(200, r#"{"Items":[]}"#),
    ]);
    assert!(reader(&fake).mint("a&b=c").await.is_err());
    assert!(fake
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with("/Auth/Keys?App=a%26b%3Dc")));
}

const ALREADY_HELD: &str = r#"{"Policy":{"EnableAllFolders":true,"EnabledFolders":["films"],"BlockUnratedItems":["Movie"],"MaxParentalRating":null}}"#;

#[tokio::test]
async fn a_choice_nobody_made_leaves_the_accounts_own_answer_standing() {
    let fake = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (Method::Post, "/Policy", Answer::reply(204, "")),
        (Method::Get, "/Users/", Answer::reply(200, ALREADY_HELD)),
    ]);

    let only_a_limit = Allowed {
        libraries: None,
        age_limit: Some(12),
        unrated: None,
    };
    assert!(
        reader(&fake)
            .allow("00000000000000000000000000000004", &only_a_limit)
            .await
            .is_ok(),
        "the account was not written"
    );

    let written: serde_json::Value = fake
        .requests()
        .into_iter()
        .find(|request| request.url.ends_with("/Policy"))
        .and_then(|request| request.body)
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_default();

    assert_eq!(
        written.get("MaxParentalRating"),
        Some(&serde_json::json!(12)),
        "the one thing chosen was not written: {written}"
    );
    assert_eq!(
        written.get("BlockUnratedItems"),
        Some(&serde_json::json!(["Movie"])),
        "an offer saying nothing about unrated content changed it anyway: {written}"
    );
    assert_eq!(
        written.get("EnableAllFolders"),
        Some(&serde_json::json!(true)),
        "an offer naming no libraries changed which ones are open: {written}"
    );
}

const DECLINE_KEYS: &str = r#"{"Items":[
  {"AppName":"lemonfiber-decline","AccessToken":"first","DateCreated":"2026-10-05T00:00:00Z","DateLastActivity":"2026-10-05T01:00:00Z"},
  {"AppName":"lemonfiber-decline","AccessToken":"second","DateCreated":"2026-10-05T02:00:00Z"},
  {"AppName":"lemonfiber-decline","AccessToken":"","DateCreated":"2026-10-05T03:00:00Z"},
  {"AppName":"Jellyseerr","AccessToken":"seerr","DateCreated":"2026-10-05T04:00:00Z","DateLastActivity":"2026-10-05T05:00:00Z"}
]}"#;

#[tokio::test]
async fn every_key_filed_under_the_decline_service_s_name_is_dated() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, DECLINE_KEYS),
    ]);

    let dated = reader(&fake).dated("lemonfiber-decline").await.ok();

    assert_eq!(
        dated,
        Some(vec![
            lemonfiber_ports::service::Dated {
                created: Some("2026-10-05T00:00:00Z".to_owned()),
                last_used: Some("2026-10-05T01:00:00Z".to_owned()),
            },
            lemonfiber_ports::service::Dated {
                created: Some("2026-10-05T02:00:00Z".to_owned()),
                last_used: None,
            },
        ])
    );
}

#[tokio::test]
async fn an_unreadable_key_list_is_no_list_of_dates() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);

    assert!(reader(&fake).dated("lemonfiber-decline").await.is_err());
}

#[tokio::test]
async fn a_key_is_proved_by_reading_with_it_alone() {
    let fake = Fake::in_turn(vec![Answer::reply(200, "{}")]);
    assert!(reader(&fake).answers_to(OURS).await.is_ok());
    let asked = fake.requests();
    assert!(asked
        .iter()
        .all(|asked| !asked.url.ends_with("/Users/AuthenticateByName")));
    assert!(asked.iter().any(|asked| asked.url.ends_with("/System/Info")
        && asked
            .headers
            .iter()
            .any(|(name, value)| name == "Authorization"
                && value == &format!(r#"MediaBrowser Token="{OURS}""#))));
    let refused = Fake::in_turn(vec![Answer::reply(401, "")]);
    assert!(matches!(
        reader(&refused).answers_to(OURS).await,
        Err(Failure::Unauthorised { .. })
    ));
}
