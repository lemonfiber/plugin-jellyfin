use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Certificate, Household as _, Library as _, NamedLibrary, Session, Unrated,
};

use super::{reader, SIGNED_IN};

const ANA: &str = "a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3";

#[tokio::test]
async fn a_rating_with_no_age_is_not_a_certificate() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(
            200,
            r#"[{"Name":"Approved"},{"Name":"PG","Value":10},{"Name":"18","Value":18}]"#,
        ),
    ]);
    assert_eq!(
        reader(&fake).ratings().await.ok(),
        Some(vec![
            Certificate {
                name: "PG".to_owned(),
                age: 10
            },
            Certificate {
                name: "18".to_owned(),
                age: 18
            },
        ])
    );
}

#[tokio::test]
async fn the_libraries_are_read_out_of_their_envelope() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Id":"l1","Name":"Films"}]}"#),
    ]);
    assert_eq!(
        reader(&fake).libraries().await.ok(),
        Some(vec![NamedLibrary {
            id: "l1".to_owned(),
            name: "Films".to_owned()
        }])
    );
}

#[tokio::test]
async fn the_devices_signed_in_are_one_account_s_alone() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(
            200,
            r#"[{"UserId":"a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3","DeviceId":"d1","DeviceName":"TV","Client":"Jellyfin Web","LastActivityDate":"2026-10-01T00:00:00Z"},
                {"UserId":"someone-else","DeviceId":"d2","DeviceName":"Phone","Client":"Swiftfin"}]"#,
        ),
    ]);
    assert_eq!(
        reader(&fake).sessions(ANA).await.ok(),
        Some(vec![Session {
            device_id: "d1".to_owned(),
            device: "TV".to_owned(),
            client: "Jellyfin Web".to_owned(),
            last_seen: Some("2026-10-01T00:00:00Z".to_owned()),
        }])
    );
}

#[tokio::test]
async fn whether_devices_sign_in_by_code_is_the_server_s_own_answer() {
    for (said, read) in [("true", true), ("false", false)] {
        let fake = Fake::in_turn(vec![
            Answer::reply(200, SIGNED_IN),
            Answer::reply(200, said),
        ]);
        assert_eq!(reader(&fake).signs_devices_in().await.ok(), Some(read));
    }
}

#[tokio::test]
async fn a_rescan_asks_every_library_to_look_again() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(204, "")]);
    assert!(reader(&fake).rescan().await.is_ok());
    assert!(fake
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Post && asked.url.ends_with("/Library/Refresh")));
    let refused = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(500, "")]);
    assert!(reader(&refused).rescan().await.is_err());
}

#[tokio::test]
async fn an_account_holding_back_any_unrated_kind_holds_back_unrated_things() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(
            200,
            r#"[{"Id":"a","Name":"ana","HasPassword":true,"Policy":{"BlockUnratedItems":["Movie"]}},
                {"Id":"b","Name":"bo","HasPassword":true,"Policy":{"BlockUnratedItems":[]}}]"#,
        ),
    ]);
    let held: Option<Vec<Unrated>> = reader(&fake).household().await.ok().map(|members| {
        members
            .into_iter()
            .map(|member| member.access.unrated)
            .collect()
    });
    assert_eq!(held, Some(vec![Unrated::HeldBack, Unrated::LetThrough]));
}
