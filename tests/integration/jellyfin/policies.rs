use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Allowed, Household as _, Unrated};

use super::{reader, SIGNED_IN};

const ANA: &str = "a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3";

const HELD: &str =
    r#"{"Policy":{"EnableAllFolders":true,"IsDisabled":true,"EnableRemoteAccess":false}}"#;

async fn written(
    edit: impl AsyncFnOnce(&jellyfin_adapter::jellyfin::Jellyfin) -> bool,
) -> serde_json::Value {
    let fake = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (Method::Post, "/Policy", Answer::reply(204, "")),
        (Method::Get, "/Users/", Answer::reply(200, HELD)),
    ]);
    assert!(edit(&reader(&fake)).await, "the edit was not accepted");
    fake.requests()
        .into_iter()
        .find(|request| request.url.ends_with(&format!("/Users/{ANA}/Policy")))
        .and_then(|request| request.body)
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_default()
}

#[tokio::test]
async fn suspending_switches_the_account_off_and_keeps_every_other_key() {
    let policy = written(async |client| client.suspend(ANA).await.is_ok()).await;
    assert_eq!(policy.get("IsDisabled"), Some(&serde_json::json!(true)));
    assert_eq!(
        policy.get("EnableRemoteAccess"),
        Some(&serde_json::json!(false))
    );
}

#[tokio::test]
async fn a_claimable_account_is_switched_on_bounded_and_narrowed() {
    let chosen = Allowed {
        libraries: Some(vec!["films".to_owned()]),
        age_limit: None,
        unrated: Some(Unrated::HeldBack),
    };
    let policy = written(async |client| client.claimable(ANA, &chosen).await.is_ok()).await;
    assert_eq!(policy.get("IsDisabled"), Some(&serde_json::json!(false)));
    assert_eq!(
        policy.get("InvalidLoginAttemptCount"),
        Some(&serde_json::json!(0))
    );
    assert_eq!(
        policy.get("LoginAttemptsBeforeLockout"),
        Some(&serde_json::json!(5))
    );
    assert_eq!(
        policy.get("EnableAllFolders"),
        Some(&serde_json::json!(false))
    );
    assert_eq!(
        policy.get("EnabledFolders"),
        Some(&serde_json::json!(["films"]))
    );
    assert_eq!(
        policy
            .get("BlockUnratedItems")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(9)
    );
    assert_eq!(
        policy.get("EnableRemoteAccess"),
        Some(&serde_json::json!(false))
    );
}

#[tokio::test]
async fn letting_unrated_things_through_holds_none_back() {
    let chosen = Allowed {
        libraries: None,
        age_limit: None,
        unrated: Some(Unrated::LetThrough),
    };
    let policy = written(async |client| client.allow(ANA, &chosen).await.is_ok()).await;
    assert_eq!(
        policy.get("BlockUnratedItems"),
        Some(&serde_json::json!([]))
    );
}
