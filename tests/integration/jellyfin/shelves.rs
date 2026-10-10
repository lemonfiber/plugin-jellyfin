use super::{reader, A_SHELF, SIGNED_IN};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::{Failure, Medium, Screening as _};

#[tokio::test]
async fn a_shelf_is_asked_for_against_the_members_own_account() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, A_SHELF),
    ]);
    let held = reader(&fake)
        .holdings(Some("a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3"), 25)
        .await;
    assert!(
        held.is_ok(),
        "a shelf the server answered was not read: {held:?}"
    );

    let asked = fake.requests();
    let url = asked
        .last()
        .map(|request| request.url.clone())
        .unwrap_or_default();
    assert!(
        url.contains("/Users/a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3/Items"),
        "the shelf was not asked for as the member: {url}"
    );
    assert!(
        url.contains("Limit=25"),
        "the count asked for did not reach the server: {url}"
    );
    assert!(
        url.contains("IncludeItemTypes=Series,Movie"),
        "the shelf asked for something other than the two kinds: {url}"
    );
    assert!(
        !url.contains(char::is_whitespace),
        "the query carries whitespace the server will not read: {url}"
    );
}

#[tokio::test]
async fn the_defaults_shelf_is_asked_for_against_no_account() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, A_SHELF),
    ]);
    let held = reader(&fake).holdings(None, 25).await;
    assert_eq!(held.map(|held| held.len()).ok(), Some(3));

    let url = fake
        .requests()
        .last()
        .map(|request| request.url.clone())
        .unwrap_or_default();
    assert!(
        url.contains("/Items?Recursive=true"),
        "the defaults shelf was not asked for: {url}"
    );
    assert!(
        !url.contains("/Users/"),
        "the defaults shelf was asked for as an account: {url}"
    );
    assert!(
        url.contains("Limit=25"),
        "the count did not reach the server: {url}"
    );
}

#[tokio::test]
async fn what_the_server_calls_things_arrives_in_this_products_words() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, A_SHELF),
    ]);
    let held = reader(&fake)
        .holdings(Some("a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3"), 25)
        .await
        .unwrap_or_default();

    assert_eq!(
        held.iter().map(|one| one.medium).collect::<Vec<_>>(),
        vec![Medium::Film, Medium::Series, Medium::Other],
        "a kind was renamed or dropped on the way in"
    );
    assert_eq!(
        held.first().map(|one| (one.title.clone(), one.year)),
        Some(("A Film".to_owned(), Some(1994)))
    );
    assert_eq!(
        held.get(1).map(|one| one.year),
        Some(None),
        "a missing year was invented"
    );
}

#[tokio::test]
async fn a_shelf_the_server_would_not_describe_is_refused_rather_than_read_as_empty() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);
    assert!(matches!(
        reader(&fake)
            .holdings(Some("a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3"), 25)
            .await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_shelf_with_nothing_on_it_is_an_answer() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "{}"),
    ]);
    assert_eq!(
        reader(&fake)
            .holdings(Some("a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3"), 25)
            .await
            .ok(),
        Some(Vec::new())
    );
}
