use std::sync::{Arc, Mutex};

use jellyfin_adapter::credential::{Credential, Holding};
use jellyfin_adapter::jellyfin::Jellyfin;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Household;

use super::{administrator, reader, BASE, HOUSEHOLD, SIGNED_IN};

fn sign_ins(fake: &Fake) -> usize {
    fake.requests()
        .iter()
        .filter(|request| request.url.ends_with("/Users/AuthenticateByName"))
        .count()
}

#[tokio::test]
async fn reads_carry_the_sign_in_they_already_made() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(200, HOUSEHOLD),
    ]);
    let client = reader(&fake);
    for _ in 0..3 {
        assert!(client.household().await.is_ok());
    }
    assert_eq!(sign_ins(&fake), 1);
}

#[tokio::test]
async fn a_token_the_server_no_longer_takes_is_minted_afresh_once() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(401, ""),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
    ]);
    let client = reader(&fake);

    assert!(client.household().await.is_ok());
    let again = client.household().await;

    assert_eq!(again.map(|held| held.len()).ok(), Some(2));
    assert_eq!(sign_ins(&fake), 2);
}

struct Replaceable(Mutex<Credential>);

impl Holding for Replaceable {
    fn credential(&self) -> Result<Credential, String> {
        self.0
            .lock()
            .map(|held| held.clone())
            .map_err(|_| "unreadable".to_owned())
    }
}

#[tokio::test]
async fn a_replaced_password_signs_in_afresh() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
    ]);
    let held = Arc::new(Replaceable(Mutex::new(administrator())));
    let client = Jellyfin::new(fake.clone(), BASE, held.clone());

    assert!(client.household().await.is_ok());
    if let Ok(mut credential) = held.0.lock() {
        credential.password = ('q'..='z').collect();
    }
    assert!(client.household().await.is_ok());

    assert_eq!(sign_ins(&fake), 2);
}

#[tokio::test]
async fn holding_no_credential_is_refused_and_nothing_is_asked() {
    let fake = Fake::silent();
    let refused = super::jellyfin(&fake).household().await;
    assert!(
        matches!(&refused, Err(lemonfiber_ports::service::Failure::Refused { detail, .. })
            if detail.contains("upstream.json")),
        "{refused:?}"
    );
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn a_sign_in_with_no_token_is_refused() {
    let fake = Fake::in_turn(vec![Answer::reply(200, "{}")]);
    assert!(reader(&fake).household().await.is_err());
}
