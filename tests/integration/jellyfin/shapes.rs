use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Allowed, Failure, Household as _, HowFar, Picture, Screening as _, Signed, Upkeep as _,
};

use super::{reader, SIGNED_IN};

const WALKING: &str = "../System/Restart";

const ITEM: &str = "f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1";

fn refused_unasked<T: std::fmt::Debug>(fake: &Fake, outcome: &Result<T, Failure>) -> bool {
    matches!(outcome, Err(Failure::Refused { .. })) && fake.requests().is_empty()
}

#[tokio::test]
async fn an_account_id_shaped_like_a_path_is_refused_before_anything_is_asked() {
    let nothing = Allowed {
        libraries: None,
        age_limit: None,
        unrated: None,
    };
    let fake = Fake::silent();
    let client = reader(&fake);
    assert!(refused_unasked(&fake, &client.withdraw(WALKING).await));
    assert!(refused_unasked(&fake, &client.unclaim(WALKING).await));
    assert!(refused_unasked(
        &fake,
        &client.allow(WALKING, &nothing).await
    ));
    assert!(refused_unasked(
        &fake,
        &client.claimable(WALKING, &nothing).await
    ));
    assert!(refused_unasked(&fake, &client.suspend(WALKING).await));
    assert!(refused_unasked(
        &fake,
        &client.holdings(Some(WALKING), 5).await
    ));
    assert!(refused_unasked(
        &fake,
        &client.signed_in(WALKING, "a-phone-0123").await
    ));
    assert!(refused_unasked(&fake, &client.part_way(WALKING, 5).await));
    assert!(refused_unasked(
        &fake,
        &client.title(Some(WALKING), ITEM).await
    ));
    assert!(refused_unasked(
        &fake,
        &client
            .progressed(
                WALKING,
                ITEM,
                &HowFar {
                    position: 1,
                    ended: false
                }
            )
            .await
    ));
}

#[tokio::test]
async fn an_item_id_shaped_like_a_path_is_no_title_and_nothing_to_record() {
    let fake = Fake::silent();
    let client = reader(&fake);
    let went = HowFar {
        position: 1,
        ended: false,
    };
    assert_eq!(client.title(None, WALKING).await.ok(), Some(None));
    assert!(refused_unasked(
        &fake,
        &client
            .progressed("a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3", WALKING, &went)
            .await
    ));
    assert!(refused_unasked(&fake, &client.refresh(WALKING).await));
}

#[tokio::test]
async fn a_device_that_could_close_its_quotes_is_refused_before_anything_is_asked() {
    let fake = Fake::silent();
    let client = reader(&fake);
    let closing = r#"x", Token="y"#;
    assert!(refused_unasked(
        &fake,
        &client.whoever("ana", "pw", closing).await
    ));
    assert!(refused_unasked(
        &fake,
        &client
            .signed_in("a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3", closing)
            .await
    ));
    assert!(refused_unasked(&fake, &client.sign_out(closing).await));
}

#[tokio::test]
async fn a_token_that_could_close_its_quotes_does_not_stand() {
    let fake = Fake::silent();
    let signed = Signed {
        id: "a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3".to_owned(),
        token: r#"t", Client="x"#.to_owned(),
    };
    assert_eq!(reader(&fake).standing(&signed).await.ok(), Some(false));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn a_moment_is_asked_for_as_a_query_value_and_adds_no_parameter() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[]}"#),
    ]);
    assert!(reader(&fake)
        .when_invited("2026-10-01T00:00:00Z&limit=1")
        .await
        .is_ok());
    assert!(fake.requests().iter().any(|asked| {
        asked.method == Method::Get
        && asked.url.ends_with(
            "/System/ActivityLog/Entries?minDate=2026-10-01T00%3A00%3A00Z%26limit%3D1&limit=200"
        )
    }));
}

#[tokio::test]
async fn a_picture_of_a_misshapen_item_is_none_and_for_a_misshapen_account_is_refused() {
    let fake = Fake::silent();
    let client = reader(&fake);
    assert_eq!(
        client.picture(None, WALKING, Picture::Poster).await.ok(),
        Some(None)
    );
    assert!(refused_unasked(
        &fake,
        &client.picture(Some(WALKING), ITEM, Picture::Poster).await
    ));
}
