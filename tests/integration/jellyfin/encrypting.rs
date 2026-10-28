use std::sync::Arc;

use super::{administrator, reader, SIGNED_IN};
use axum::routing::get;
use axum::Router;
use jellyfin_adapter::jellyfin::Jellyfin;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::{Http, Method};
use lemonfiber_ports::service::{Applied, Encrypting as _};

const PATH: &str = "/config/lemonfiber/internal.p12";

fn pin() -> String {
    "ab".repeat(32)
}

fn internal_tls() -> Option<lemonfiber_pinned::fake::Served> {
    lemonfiber_pinned::fake::tls(Router::new().route("/", get(|| async { "" })))
}

const PLAIN: &str = r#"{"EnableHttps":false,"RequireHttps":false,"CertificatePath":"","CertificatePassword":"","InternalHttpPort":8096,"InternalHttpsPort":8920,"KnownProxies":["10.0.0.2"]}"#;

const SERVING: &str = r#"{"EnableHttps":true,"RequireHttps":false,"CertificatePath":"/config/lemonfiber/internal.p12","CertificatePassword":"","InternalHttpPort":8096,"InternalHttpsPort":8920,"KnownProxies":["10.0.0.2"]}"#;

fn posted(fake: &Fake) -> Vec<serde_json::Value> {
    fake.requests()
        .into_iter()
        .filter(|asked| {
            asked.method == Method::Post && asked.url.ends_with("/System/Configuration/network")
        })
        .filter_map(|asked| asked.body.and_then(|body| serde_json::from_str(&body).ok()))
        .collect()
}

#[tokio::test]
async fn the_internal_tls_port_is_turned_on_with_the_certificate_and_in_force_once_jellyfin_restarts(
) {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, PLAIN),
        Answer::reply(204, ""),
        Answer::reply(200, SERVING),
    ]);

    let applied = reader(&fake).serve_tls(PATH, &pin()).await;

    assert_eq!(applied.ok(), Some(Applied::OnRestart));
    assert_eq!(
        posted(&fake),
        vec![serde_json::from_str::<serde_json::Value>(SERVING).unwrap_or_default()]
    );
}

#[tokio::test]
async fn a_certificate_the_internal_port_already_presents_is_in_force_now_and_nothing_is_written() {
    let served = internal_tls();
    assert!(served.is_some());
    let Some(served) = served else { return };
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SERVING),
    ]);

    let applied = reader(&fake)
        .with_internal_tls(served.base)
        .serve_tls(PATH, &served.pin)
        .await;

    assert_eq!(applied.ok(), Some(Applied::Now));
    assert!(posted(&fake).is_empty());
}

#[tokio::test]
async fn a_certificate_named_but_not_yet_presented_is_in_force_once_jellyfin_restarts() {
    let served = internal_tls();
    assert!(served.is_some());
    let Some(served) = served else { return };
    for at in [Some(served.base), None] {
        let fake = Fake::in_turn(vec![
            Answer::reply(200, SIGNED_IN),
            Answer::reply(200, SERVING),
        ]);
        let jellyfin = reader(&fake);
        let jellyfin = match at {
            Some(at) => jellyfin.with_internal_tls(at),
            None => jellyfin,
        };

        let applied = jellyfin.serve_tls(PATH, &pin()).await;

        assert_eq!(applied.ok(), Some(Applied::OnRestart));
        assert!(posted(&fake).is_empty());
    }
}

#[tokio::test]
async fn a_jellyfin_at_no_address_is_never_taken_to_present_a_certificate() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SERVING),
    ]);
    let http: Arc<dyn Http> = fake.clone();
    let jellyfin = Jellyfin::new(http, "not an address", Arc::new(administrator()));

    assert_eq!(
        jellyfin.serve_tls(PATH, &pin()).await.ok(),
        Some(Applied::OnRestart)
    );
}

#[tokio::test]
async fn settings_that_read_back_otherwise_are_a_failure() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, PLAIN),
        Answer::reply(204, ""),
        Answer::reply(200, PLAIN),
    ]);

    let applied = reader(&fake).serve_tls(PATH, &pin()).await;

    assert!(
        applied
            .as_ref()
            .is_err_and(|failure| failure.to_string().contains("read back otherwise")),
        "{applied:?}"
    );
}

#[tokio::test]
async fn settings_that_are_not_an_object_or_a_write_refused_are_failures() {
    for answers in [
        vec![Answer::reply(200, SIGNED_IN), Answer::reply(200, "[]")],
        vec![
            Answer::reply(200, SIGNED_IN),
            Answer::reply(200, PLAIN),
            Answer::reply(400, ""),
        ],
    ] {
        let fake = Fake::in_turn(answers);

        assert!(reader(&fake).serve_tls(PATH, &pin()).await.is_err());
    }
}

#[tokio::test]
async fn an_empty_path_or_a_pin_that_is_no_sha256_is_refused_without_asking_jellyfin() {
    let refused = [
        (String::new(), pin()),
        ("  ".to_owned(), pin()),
        (PATH.to_owned(), "AB".repeat(32)),
        (PATH.to_owned(), "ab".repeat(31)),
        (PATH.to_owned(), "not a pin".to_owned()),
    ];
    for (path, pin) in refused {
        let fake = Fake::in_turn(Vec::new());

        assert!(reader(&fake).serve_tls(&path, &pin).await.is_err());
        assert!(fake.requests().is_empty());
    }
}
