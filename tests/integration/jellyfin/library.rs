use super::{reader, SIGNED_IN};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::Failure;
use lemonfiber_ports::service::Library;

#[tokio::test]
async fn a_present_title_signs_in_then_finds_it_in_the_library() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Name":"The Expanse"}]}"#),
    ]);
    assert_eq!(
        reader(&fake).has_item(Kind::Tv, "expanse").await.ok(),
        Some(true)
    );

    let requests = fake.requests();
    assert!(requests.first().is_some_and(|request| {
        request.url.ends_with("/Users/AuthenticateByName")
            && request
                .headers
                .iter()
                .any(|(name, value)| name == "Authorization" && value.contains("lemonfiber"))
            && request.body.as_deref().is_some_and(|body| {
                body.contains(r#""Username":"admin""#) && body.contains(r#""Pw":""#)
            })
    }));
    assert!(requests.get(1).is_some_and(|request| {
        request.url.contains("IncludeItemTypes=Series")
            && request.headers.iter().any(|(name, value)| {
                name == "Authorization" && value == r#"MediaBrowser Token="token""#
            })
    }));
}

#[tokio::test]
async fn a_library_without_the_title_reads_as_absent() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Name":"Some Other Show"}]}"#),
    ]);
    assert_eq!(
        reader(&fake).has_item(Kind::Tv, "expanse").await.ok(),
        Some(false)
    );
}

#[tokio::test]
async fn a_film_read_asks_the_library_for_movies() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Name":"Dune"}]}"#),
    ]);
    assert_eq!(
        reader(&fake).has_item(Kind::Movies, "dune").await.ok(),
        Some(true)
    );
    assert!(fake
        .requests()
        .get(1)
        .is_some_and(|read| read.url.contains("IncludeItemTypes=Movie")));
}

#[tokio::test]
async fn a_refused_sign_in_fails_before_any_library_read() {
    let fake = Fake::in_turn(vec![Answer::reply(401, "")]);
    assert!(matches!(
        reader(&fake).has_item(Kind::Tv, "expanse").await,
        Err(Failure::Unauthorised { .. })
    ));
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn an_unreadable_library_is_refused() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);
    assert!(matches!(
        reader(&fake).has_item(Kind::Tv, "expanse").await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_media_server_is_unavailable_for_a_library_read() {
    let fake = Fake::silent();
    assert!(matches!(
        reader(&fake).has_item(Kind::Tv, "expanse").await,
        Err(Failure::Unavailable { .. })
    ));
}
