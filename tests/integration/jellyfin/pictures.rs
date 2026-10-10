use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Picture, Screening as _};

use super::{reader, SIGNED_IN};

const FILM: &str = "f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1";

const ANA: &str = "a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3";

const THE_FILM: &str = r#"{"Id":"f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1","Name":"Heat","Type":"Movie"}"#;

#[tokio::test]
async fn a_poster_the_member_may_see_is_answered_with_its_media_type() {
    let fake = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (
            Method::Get,
            "/Images/Primary",
            Answer::served(200, "image/jpeg", "jpeg-bytes"),
        ),
        (Method::Get, "/Items/", Answer::reply(200, THE_FILM)),
    ]);
    let image = reader(&fake)
        .picture(Some(ANA), FILM, Picture::Poster)
        .await
        .ok()
        .flatten();
    assert_eq!(
        image.map(|image| (image.media_type, image.bytes)),
        Some(("image/jpeg".to_owned(), b"jpeg-bytes".to_vec()))
    );
}

#[tokio::test]
async fn a_picture_of_a_title_the_member_may_not_see_is_never_asked_for() {
    let fake = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, SIGNED_IN),
        ),
        (Method::Get, "/Items/", Answer::reply(404, "")),
    ]);
    let image = reader(&fake)
        .picture(Some(ANA), FILM, Picture::Backdrop)
        .await;
    assert!(matches!(image, Ok(None)), "{image:?}");
    assert!(!fake
        .requests()
        .iter()
        .any(|asked| asked.url.contains("/Images/")));
}

#[tokio::test]
async fn a_picture_the_server_does_not_hold_is_none_and_one_it_will_not_answer_is_refused() {
    for (status, absent) in [(404, true), (500, false)] {
        let fake = Fake::by_route(vec![
            (
                Method::Post,
                "/Users/AuthenticateByName",
                Answer::reply(200, SIGNED_IN),
            ),
            (Method::Get, "/Images/Backdrop", Answer::reply(status, "")),
            (Method::Get, "/Items/", Answer::reply(200, THE_FILM)),
        ]);
        let image = reader(&fake).picture(None, FILM, Picture::Backdrop).await;
        assert_eq!(
            matches!(image, Ok(None)),
            absent,
            "{status} was read as {image:?}"
        );
        assert_eq!(image.is_err(), !absent, "{status} was read as {image:?}");
    }
}

#[tokio::test]
async fn a_device_is_signed_out_by_its_own_id() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(204, "")]);
    assert!(reader(&fake).sign_out("a-phone-0123").await.is_ok());
    assert!(fake
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Delete
            && asked.url.ends_with("/Devices?id=a-phone-0123")));
}

#[tokio::test]
async fn a_picture_nothing_answers_is_unavailable() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, THE_FILM),
    ]);
    let unanswered = reader(&fake).picture(None, FILM, Picture::Poster).await;
    assert!(
        matches!(
            unanswered,
            Err(lemonfiber_ports::service::Failure::Unavailable { .. })
        ),
        "{unanswered:?}"
    );
}
