use std::sync::Arc;

use super::jellyfin;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_ports::service::{Failure, Recounting as _};

#[tokio::test]
async fn asked_to_tell_the_core_what_happened_it_says_it_cannot_yet_and_asks_jellyfin_nothing() {
    let fake = Arc::new(Fake::in_turn(Vec::new()));
    let said = jellyfin(&fake)
        .events(
            Some("https://core.invalid/api/notify/events"),
            Some(&"ab".repeat(32)),
            Some("key"),
        )
        .await;
    assert!(
        matches!(&said, Err(Failure::Unsupported { service, detail }) if service == "jellyfin" && detail.contains("Webhook plugin")),
        "{said:?}"
    );
    assert!(fake.requests().is_empty());
}
