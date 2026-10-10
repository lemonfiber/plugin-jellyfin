use std::path::PathBuf;

use super::{Credential, Holding, Upstream, UPSTREAM_FILE};

fn directory(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-adapter-credential-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn a_password() -> String {
    ('d'..='q').collect()
}

#[test]
fn the_file_lemonfiber_wrote_is_read_as_the_credential() {
    let dir = directory("read");
    let password = a_password();
    let _ = std::fs::write(
        dir.join(UPSTREAM_FILE),
        format!(r#"{{"username":"admin","password":"{password}"}}"#),
    );
    assert_eq!(
        Upstream::in_directory(&dir).credential(),
        Ok(Credential {
            username: "admin".to_owned(),
            password,
        })
    );
}

#[test]
fn a_missing_file_is_refused_by_name() {
    let dir = directory("missing");
    let refused = Upstream::in_directory(&dir).credential();
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.starts_with("upstream.json could not be opened")),
        "{refused:?}"
    );
}

#[test]
fn an_unreadable_file_is_refused_without_quoting_it() {
    let dir = directory("unreadable");
    let password = a_password();
    let _ = std::fs::write(dir.join(UPSTREAM_FILE), format!("password={password}"));
    let refused = Upstream::in_directory(&dir).credential();
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| !why.contains(&password) && why.contains("not an object")),
        "{refused:?}"
    );
}

#[test]
fn an_empty_password_is_no_credential() {
    let dir = directory("empty");
    let _ = std::fs::write(
        dir.join(UPSTREAM_FILE),
        r#"{"username":"admin","password":""}"#,
    );
    assert!(Upstream::in_directory(&dir).credential().is_err());
}

#[test]
fn a_file_past_the_bound_is_refused() {
    let dir = directory("large");
    let _ = std::fs::write(dir.join(UPSTREAM_FILE), " ".repeat(5000));
    let refused = Upstream::in_directory(&dir).credential();
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.contains("larger than")),
        "{refused:?}"
    );
}

#[test]
fn a_credential_never_prints_its_password() {
    let password = a_password();
    let held = Credential {
        username: "admin".to_owned(),
        password: password.clone(),
    };
    assert!(!format!("{held:?}").contains(&password));
}

#[test]
fn a_file_that_cannot_be_read_is_refused_by_name() {
    let dir = directory("directory");
    let _ = std::fs::create_dir_all(dir.join(UPSTREAM_FILE));
    let refused = Upstream::in_directory(&dir).credential();
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.starts_with("upstream.json could not be")),
        "{refused:?}"
    );
}
