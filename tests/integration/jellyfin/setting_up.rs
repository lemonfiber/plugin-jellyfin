use super::{a_word, jellyfin, reader};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::Failure;
use lemonfiber_ports::service::MediaServer;

#[tokio::test]
async fn a_completed_wizard_is_reported() {
    let fake = Fake::in_turn(vec![Answer::reply(
        200,
        r#"{"StartupWizardCompleted":true}"#,
    )]);
    assert_eq!(jellyfin(&fake).startup_completed().await.ok(), Some(true));
    assert!(fake
        .requests()
        .first()
        .is_some_and(|request| request.url.ends_with("/System/Info/Public")));
}

#[tokio::test]
async fn an_incomplete_or_unstated_wizard_reads_as_not_done() {
    let fake = Fake::in_turn(vec![Answer::reply(
        200,
        r#"{"StartupWizardCompleted":false}"#,
    )]);
    assert_eq!(jellyfin(&fake).startup_completed().await.ok(), Some(false));
    let bare = Fake::in_turn(vec![Answer::reply(200, "{}")]);
    assert_eq!(jellyfin(&bare).startup_completed().await.ok(), Some(false));
}

#[tokio::test]
async fn an_unreadable_public_info_is_refused() {
    let fake = Fake::in_turn(vec![Answer::reply(200, "not json")]);
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_refused_public_info_carries_the_status() {
    let fake = Fake::in_turn(vec![Answer::reply(500, "")]);
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Refused { detail, .. }) if detail == "HTTP 500"
    ));
}

#[tokio::test]
async fn a_jellyfin_still_starting_is_unavailable() {
    let fake = Fake::in_turn(vec![Answer::reply(503, "<html>starting</html>")]);
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_jellyfin_is_unavailable() {
    let fake = Fake::silent();
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn the_account_is_read_into_being_before_it_is_written() {
    let password = a_word();
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Name":"root"}"#),
        Answer::reply(204, ""),
        Answer::reply(204, ""),
    ]);
    assert!(jellyfin(&fake)
        .create_admin("admin", &password)
        .await
        .is_ok());

    let requests = fake.requests();
    let steps: Vec<(Method, &str)> = requests
        .iter()
        .map(|request| {
            let path = request
                .url
                .rsplit_once("/api")
                .map_or(request.url.as_str(), |(_, rest)| rest);
            (request.method, path)
        })
        .collect();
    assert!(
        matches!(
            steps.as_slice(),
            [
                (Method::Get, first),
                (Method::Post, second),
                (Method::Post, third)
            ] if first.ends_with("/Startup/User")
                && second.ends_with("/Startup/User")
                && third.ends_with("/Startup/Complete")
        ),
        "the account was not read before it was written: {steps:?}"
    );

    let written = requests
        .get(1)
        .and_then(|request| request.body.clone())
        .unwrap_or_default();
    assert!(
        written.contains(r#""Name":"admin""#),
        "the account written is not the one asked for"
    );
    assert!(
        written.contains(&format!(r#""Password":"{password}""#)),
        "the password written is not the one asked for"
    );
}

#[tokio::test]
async fn a_rejected_admin_creation_is_refused_and_setup_is_not_finished() {
    let password = a_word();
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Name":"root"}"#),
        Answer::reply(400, "user already exists"),
    ]);
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", &password).await,
        Err(Failure::Refused { .. })
    ));
    assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn a_rejected_completion_is_refused() {
    let password = a_word();
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Name":"root"}"#),
        Answer::reply(204, ""),
        Answer::reply(500, "boom"),
    ]);
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", &password).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn creating_the_admin_on_an_unreachable_jellyfin_is_unavailable() {
    let password = a_word();
    let fake = Fake::silent();
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", &password).await,
        Err(Failure::Unavailable { .. })
    ));
}

const ADMIN_SIGNED_IN: &str =
    r#"{"AccessToken":"token","User":{"Id":"a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3"}}"#;

#[tokio::test]
async fn the_password_is_changed_with_the_one_in_force_and_proved_with_the_replacement() {
    let replacement = a_word();
    let fake = Fake::in_turn(vec![
        Answer::reply(200, ADMIN_SIGNED_IN),
        Answer::reply(204, ""),
        Answer::reply(200, ADMIN_SIGNED_IN),
    ]);
    assert!(reader(&fake)
        .replace_admin_password(&replacement)
        .await
        .is_ok());
    let requests = fake.requests();
    let in_force = super::administrator().password;
    let bodies: Vec<(String, String)> = requests
        .iter()
        .map(|asked| (asked.url.clone(), asked.body.clone().unwrap_or_default()))
        .collect();
    assert!(
        matches!(
            bodies.as_slice(),
            [(first, signing), (second, changing), (third, proving)]
                if first.ends_with("/Users/AuthenticateByName") && signing.contains(&in_force)
                && second.ends_with("/Users/a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3/Password")
                && changing.contains(&format!(r#""CurrentPw":"{in_force}""#))
                && changing.contains(&format!(r#""NewPw":"{replacement}""#))
                && third.ends_with("/Users/AuthenticateByName") && proving.contains(&replacement)
        ),
        "{bodies:?}"
    );
    assert!(requests
        .get(1)
        .is_some_and(|changing| changing.headers.iter().any(|(name, value)| name
            == "Authorization"
            && value == r#"MediaBrowser Token="token""#)));
}

#[tokio::test]
async fn a_replacement_the_sign_in_refuses_is_refused_and_not_unauthorised() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, ADMIN_SIGNED_IN),
        Answer::reply(204, ""),
        Answer::reply(401, ""),
    ]);
    assert!(matches!(
        reader(&fake).replace_admin_password(&a_word()).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_password_in_force_jellyfin_refuses_is_unauthorised_and_nothing_is_changed() {
    let fake = Fake::in_turn(vec![Answer::reply(401, "")]);
    assert!(matches!(
        reader(&fake).replace_admin_password(&a_word()).await,
        Err(Failure::Unauthorised { .. })
    ));
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_change_jellyfin_refuses_is_not_proved() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, ADMIN_SIGNED_IN),
        Answer::reply(400, "bad"),
    ]);
    assert!(matches!(
        reader(&fake).replace_admin_password(&a_word()).await,
        Err(Failure::Refused { .. })
    ));
    assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn an_empty_replacement_or_an_account_with_no_id_changes_nothing() {
    let fake = Fake::silent();
    assert!(matches!(
        reader(&fake).replace_admin_password("").await,
        Err(Failure::Refused { .. })
    ));
    assert!(fake.requests().is_empty());
    let nameless = Fake::in_turn(vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)]);
    assert!(matches!(
        reader(&nameless).replace_admin_password(&a_word()).await,
        Err(Failure::Refused { .. })
    ));
    assert_eq!(nameless.requests().len(), 1);
}

#[tokio::test]
async fn holding_no_credential_changes_nothing() {
    let fake = Fake::silent();
    assert!(matches!(
        jellyfin(&fake).replace_admin_password(&a_word()).await,
        Err(Failure::Refused { .. })
    ));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn a_proof_nothing_answers_is_unavailable() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, ADMIN_SIGNED_IN),
        Answer::reply(204, ""),
    ]);
    assert!(matches!(
        reader(&fake).replace_admin_password(&a_word()).await,
        Err(Failure::Unavailable { .. })
    ));
}
