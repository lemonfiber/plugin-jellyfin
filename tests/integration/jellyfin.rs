use lemonfiber_fixtures::http::Fake;
use std::sync::Arc;

use jellyfin_adapter::credential::{Credential, Holding};
use jellyfin_adapter::jellyfin::Jellyfin;
use lemonfiber_ports::http::Http;

const BASE: &str = "http://127.0.0.1:8096";

struct Nothing;

impl Holding for Nothing {
    fn credential(&self) -> Result<Credential, String> {
        Err("upstream.json could not be opened: not found".to_owned())
    }
}

fn a_word() -> String {
    ('g'..='w').collect()
}

fn jellyfin(fake: &Arc<Fake>) -> Jellyfin {
    let http: Arc<dyn Http> = fake.clone();
    Jellyfin::new(http, BASE, Arc::new(Nothing))
}

fn administrator() -> Credential {
    Credential {
        username: "admin".to_owned(),
        password: ('a'..='p').collect(),
    }
}

fn reader(fake: &Arc<Fake>) -> Jellyfin {
    let http: Arc<dyn Http> = fake.clone();
    Jellyfin::new(http, BASE, Arc::new(administrator()))
}

const SIGNED_IN: &str = r#"{"AccessToken":"token"}"#;

const HOUSEHOLD: &str = r#"[
    {"Id":"1","Name":"ana","HasPassword":false},
    {"Id":"2","Name":"bo","HasPassword":true}
]"#;

const OURS: &str = "abababababababababababababababab";

const OURS_TOO: &str = r#"{"Items":[{"AppName":"Seerr","AccessToken":"theirs"},{"AppName":"lemonfiber","AccessToken":"abababababababababababababababab"}],"TotalRecordCount":2}"#;

fn hers() -> String {
    ('b'..='m').collect()
}

fn not_hers() -> String {
    ('c'..='n').rev().collect()
}

const SIGNED_IN_AS: &str =
    r#"{"AccessToken":"token","User":{"Id":"a7f3a7f3a7f3a7f3a7f3a7f3a7f3a7f3","Name":"ana"}}"#;

const A_SHELF: &str = r#"{"Items":[
    {"Id":"one","Name":"A Film","ProductionYear":1994,"Type":"Movie"},
    {"Id":"two","Name":"A Series","Type":"Series"},
    {"Id":"three","Name":"An Album","ProductionYear":1973,"Type":"MusicAlbum"}
]}"#;

mod accounts;
mod cors;
mod declaring;
mod encrypting;
mod keys;
mod library;
mod pictures;
mod playing;
mod policies;
mod proxies;
mod reads;
mod screening;
mod sessions;
mod setting_up;
mod shapes;
mod shelves;
mod upkeep;
