use super::{hers, jellyfin, not_hers, reader, HOUSEHOLD, SIGNED_IN, SIGNED_IN_AS};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::Household;
use lemonfiber_ports::service::{Failure, Signed};
use std::sync::Arc;

#[tokio::test]
async fn an_account_without_a_password_reads_as_unclaimed() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
    ]);

    let held = reader(&fake).household().await.unwrap_or_default();

    assert_eq!(
        held.iter()
            .map(|member| (member.name.as_str(), member.claimed))
            .collect::<Vec<_>>(),
        [("ana", false), ("bo", true)],
        "claimed and unclaimed were not told apart"
    );
}

#[tokio::test]
async fn offering_an_account_sends_no_password() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Id":"1","Name":"ana","HasPassword":false}"#),
    ]);

    let made = reader(&fake).invite("ana").await;

    assert!(made.is_ok_and(|member| member.name == "ana" && !member.claimed));
    let sent = fake
        .requests()
        .into_iter()
        .find(|asked| asked.url.ends_with("/Users/New"))
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        sent.contains(r#""Name":"ana""#) && !sent.to_lowercase().contains("password"),
        "a password was sent with an invitation: {sent}"
    );
}

#[tokio::test]
async fn withdrawing_an_invitation_removes_the_account() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(204, "")]);

    assert!(reader(&fake)
        .withdraw("00000000000000000000000000000001")
        .await
        .is_ok());
    assert!(
        fake.requests()
            .iter()
            .any(|asked| asked.method == Method::Delete
                && asked
                    .url
                    .ends_with("/Users/00000000000000000000000000000001")),
        "the account was not asked to be removed: {:?}",
        fake.requests()
    );
}

#[tokio::test]
async fn resetting_a_password_sends_a_flag_and_never_a_password() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(204, "")]);

    assert!(reader(&fake)
        .unclaim("00000000000000000000000000000001")
        .await
        .is_ok());

    let body = fake
        .requests()
        .into_iter()
        .find(|asked| {
            asked.method == Method::Post
                && asked
                    .url
                    .ends_with("/Users/00000000000000000000000000000001/Password")
        })
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        body.contains(r#""ResetPassword":true"#),
        "no reset was asked for at the account's own password: {body:?}"
    );
    assert!(
        !body.to_lowercase().contains("pw") && body.matches(':').count() == 1,
        "the reset carried more than the flag, so a password could be among it: {body}"
    );
}

#[tokio::test]
async fn a_refused_reset_is_not_reported_as_done() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(403, "")]);

    assert!(
        reader(&fake)
            .unclaim("00000000000000000000000000000001")
            .await
            .is_err(),
        "a refused reset was reported as having happened"
    );
}

#[tokio::test]
async fn when_an_account_was_made_is_read_from_what_the_server_recorded() {
    let recorded = r#"{"Items":[
        {"Type":"UserCreated","Date":"2026-08-29T09:00:00Z","UserId":"1"},
        {"Type":"UserPasswordChanged","Date":"2026-08-30T09:00:00Z","UserId":"1"},
        {"Type":"AuthenticationSucceeded","Date":"2026-08-29T10:00:00Z","UserId":"2"},
        {"Type":"SessionStarted","Date":"2026-08-29T10:00:01Z","UserId":"2"},
        {"Type":"UserCreated","Date":"2026-08-29T11:00:00Z","UserId":""}
    ]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, recorded),
    ]);

    let made = reader(&fake)
        .when_invited("2026-08-27T09:00:00Z")
        .await
        .unwrap_or_default();

    assert_eq!(
        made.iter()
            .map(|entry| (entry.member.as_str(), entry.at.as_str()))
            .collect::<Vec<_>>(),
        [("1", "2026-08-29T09:00:00Z"), ("1", "2026-08-30T09:00:00Z")],
        "either an account being offered was dropped, or something that is not an \
         offering was carried through"
    );
    assert!(
        fake.requests().iter().any(
            |asked| asked.url.contains("minDate=2026-08-27T09%3A00%3A00Z")
                || asked.url.contains("minDate=2026-08-27T09:00:00Z")
        ),
        "the read was not bounded by the date asked for: {:?}",
        fake.requests()
    );
}

#[tokio::test]
async fn a_media_server_that_will_not_answer_is_unavailable_for_the_household() {
    let fake = Fake::silent();
    assert!(matches!(
        reader(&fake).household().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_pair_the_server_knows_answers_with_the_account_it_proved() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN_AS)]);

    assert_eq!(
        jellyfin(&fake)
            .whoever("ana", &hers(), "this-browser")
            .await
            .ok()
            .flatten(),
        Some(Signed {
            id: "a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3".to_owned(),
            token: "token".to_owned()
        })
    );
    let asked: Vec<&str> = fake
        .requests()
        .iter()
        .map(|request| request.url.as_str())
        .filter(|url| url.ends_with("/Users/AuthenticateByName"))
        .map(|_| "authenticate")
        .collect();
    assert_eq!(asked, ["authenticate"], "the member was proved elsewhere");
}

#[tokio::test]
async fn a_pair_the_server_refuses_is_nobody_rather_than_a_fault() {
    for refused in [401, 403] {
        let fake = Fake::in_turn(vec![Answer::reply(refused, r#"{"error":"no"}"#)]);

        let said = jellyfin(&fake)
            .whoever("ana", &not_hers(), "this-browser")
            .await;
        assert!(
            matches!(said, Ok(None)),
            "a refusal at {refused} was read as something other than nobody"
        );
    }
}

#[tokio::test]
async fn a_server_that_could_not_answer_is_not_a_pair_that_was_wrong() {
    let fake = Fake::in_turn(vec![Answer::reply(500, "upstream is unwell")]);

    assert!(
        jellyfin(&fake)
            .whoever("ana", &hers(), "this-browser")
            .await
            .is_err(),
        "a server that could not answer was read as a pair that was wrong"
    );
}

#[tokio::test]
async fn an_account_with_no_id_is_nobody() {
    let fake = Fake::in_turn(vec![Answer::reply(200, r#"{"AccessToken":"t","User":{}}"#)]);

    assert!(matches!(
        jellyfin(&fake)
            .whoever("ana", &hers(), "this-browser")
            .await,
        Ok(None)
    ));
}

const STILL_HELD: &str = r#"{"Id":"a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3","Name":"ana","HasPassword":true,"Policy":{"IsDisabled":false}}"#;

const SWITCHED_OFF: &str = r#"{"Id":"a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3","Name":"ana","HasPassword":true,"Policy":{"IsDisabled":true}}"#;

fn signed() -> Signed {
    Signed {
        id: "a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3".to_owned(),
        token: ('m'..='z').collect(),
    }
}

fn as_the_member(answer: Answer) -> Arc<Fake> {
    Fake::by_route(vec![(Method::Get, "/Users/Me", answer)])
}

#[tokio::test]
async fn a_sign_in_the_server_still_honours_is_standing() {
    let fake = as_the_member(Answer::reply(200, STILL_HELD));

    assert!(matches!(reader(&fake).standing(&signed()).await, Ok(true)));
    let carried: Vec<String> = fake
        .requests()
        .iter()
        .flat_map(|request| request.headers.iter())
        .filter(|(name, _)| name == "Authorization")
        .map(|(_, value)| value.clone())
        .collect();
    assert_eq!(
        carried,
        [format!(r#"MediaBrowser Token="{}""#, signed().token)]
    );
}

#[tokio::test]
async fn a_sign_in_the_server_no_longer_honours_is_not_standing() {
    let fake = as_the_member(Answer::reply(401, ""));

    assert!(matches!(reader(&fake).standing(&signed()).await, Ok(false)));
}

#[tokio::test]
async fn an_account_switched_off_at_the_server_is_not_standing() {
    let fake = as_the_member(Answer::reply(200, SWITCHED_OFF));

    assert!(matches!(reader(&fake).standing(&signed()).await, Ok(false)));
}

#[tokio::test]
async fn an_account_offered_again_is_not_standing() {
    let fake = as_the_member(Answer::reply(
        200,
        r#"{"Id":"a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3","Name":"ana","HasPassword":false}"#,
    ));

    assert!(matches!(reader(&fake).standing(&signed()).await, Ok(false)));
}

#[tokio::test]
async fn access_answered_for_another_account_is_not_standing() {
    let fake = as_the_member(Answer::reply(
        200,
        r#"{"Id":"b8e4","Name":"ben","HasPassword":true}"#,
    ));

    assert!(matches!(reader(&fake).standing(&signed()).await, Ok(false)));
}

#[tokio::test]
async fn a_server_that_would_not_say_is_not_an_account_that_is_gone() {
    let fake = as_the_member(Answer::reply(500, "upstream is unwell"));

    assert!(
        reader(&fake).standing(&signed()).await.is_err(),
        "a server that could not answer was read as an account that is gone"
    );
}

#[tokio::test]
async fn an_account_answered_without_a_policy_is_one_the_server_still_holds() {
    let fake = as_the_member(Answer::reply(
        200,
        r#"{"Id":"a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3","Name":"ana","HasPassword":true}"#,
    ));

    assert!(matches!(reader(&fake).standing(&signed()).await, Ok(true)));
}

#[tokio::test]
async fn a_sign_in_is_named_as_the_device_the_caller_chose() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN_AS)]);
    let _ = jellyfin(&fake)
        .whoever("ana", &hers(), "this-browser")
        .await;
    let named = fake
        .requests()
        .iter()
        .flat_map(|request| request.headers.iter())
        .any(|(name, value)| {
            name == "Authorization" && value.contains(r#"DeviceId="this-browser""#)
        });
    assert!(named, "{:?}", fake.requests());
}

#[tokio::test]
async fn a_sign_in_granted_no_access_is_nobody() {
    let fake = Fake::in_turn(vec![Answer::reply(
        200,
        r#"{"User":{"Id":"a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3"}}"#,
    )]);

    assert!(matches!(
        jellyfin(&fake)
            .whoever("ana", &hers(), "this-browser")
            .await,
        Ok(None)
    ));
}

#[tokio::test]
async fn a_claim_sets_the_chosen_password_signed_in_as_the_account() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN_AS),
        Answer::reply(204, ""),
        Answer::reply(204, ""),
    ]);

    let claimed = jellyfin(&fake).claim("ana", &hers(), "a-phone-0123").await;

    assert!(matches!(claimed, Ok(true)), "{claimed:?}");
    let asked = fake.requests();
    assert_eq!(asked.len(), 3);
    assert!(asked
        .first()
        .and_then(|request| request.body.as_deref())
        .is_some_and(|body| body.contains(r#""Pw":"""#)));
    let changing = asked.get(1);
    assert!(changing.is_some_and(|request| {
        request
            .url
            .ends_with("/Users/a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3/Password")
            && request
                .body
                .as_deref()
                .is_some_and(|body| body == format!(r#"{{"CurrentPw":"","NewPw":"{}"}}"#, hers()))
            && request
                .headers
                .iter()
                .any(|(name, value)| name == "Authorization" && value.contains(r#"Token="token""#))
    }));
    assert!(asked
        .get(2)
        .is_some_and(|request| request.url.ends_with("/Sessions/Logout")));
}

#[tokio::test]
async fn an_account_that_will_not_sign_in_empty_is_not_claimed() {
    let fake = Fake::in_turn(vec![Answer::reply(401, "")]);

    assert!(matches!(
        jellyfin(&fake).claim("ana", &hers(), "a-phone-0123").await,
        Ok(false)
    ));
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_password_the_server_will_not_take_is_a_failure() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN_AS),
        Answer::reply(400, ""),
    ]);

    assert!(jellyfin(&fake)
        .claim("ana", &hers(), "a-phone-0123")
        .await
        .is_err());
}
