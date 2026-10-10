use super::super::Jellyfin;
use super::AccountResource;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Allowed, Failure, Unrated};

const EVERY_LIBRARY: &str = "EnableAllFolders";

const CHOSEN_LIBRARIES: &str = "EnabledFolders";

const AGE_LIMIT: &str = "MaxParentalRating";

const UNRATED_KINDS: [&str; 9] = [
    "Movie",
    "Trailer",
    "Series",
    "Music",
    "Book",
    "LiveTvChannel",
    "LiveTvProgram",
    "ChannelContent",
    "Other",
];

const UNRATED: &str = "BlockUnratedItems";

pub(super) enum Edit<'a> {
    /// What the operator chose it may watch.
    Allow(&'a Allowed),
    /// Switched on, its wrong passwords forgotten, bounded from here on, and narrowed to
    /// what was chosen.
    Claimable(&'a Allowed),
    /// Switched off, and kept.
    Suspend,
}

pub(super) async fn rewritten(
    jellyfin: &Jellyfin,
    id: &str,
    edit: &Edit<'_>,
) -> Result<(), Failure> {
    if !crate::shape::an_id(id) {
        return Err(jellyfin.misshapen("an account"));
    }
    let response = jellyfin
        .as_admin(Method::Get, &format!("/Users/{id}"), None)
        .await?;
    let held: AccountResource = jellyfin
        .endpoint
        .decode(&response, "what the account is allowed could not be read")?;

    let mut policy = held.policy;
    edited(&mut policy, edit);

    let body = serde_json::Value::Object(policy).to_string();
    let response = jellyfin
        .as_admin(Method::Post, &format!("/Users/{id}/Policy"), Some(body))
        .await?;
    jellyfin.endpoint.expect_success(&response)
}

fn edited(policy: &mut serde_json::Map<String, serde_json::Value>, edit: &Edit<'_>) {
    match edit {
        Edit::Allow(allowed) => narrowed(policy, allowed),
        Edit::Claimable(allowed) => {
            narrowed(policy, allowed);
            policy.insert(DISABLED.to_owned(), false.into());
            policy.insert(WRONG_SO_FAR.to_owned(), 0.into());
            policy.insert(WRONG_BEFORE_LOCKOUT.to_owned(), WRONG_ALLOWED.into());
        }
        Edit::Suspend => {
            policy.insert(DISABLED.to_owned(), true.into());
        }
    }
}

fn narrowed(policy: &mut serde_json::Map<String, serde_json::Value>, allowed: &Allowed) {
    if let Some(libraries) = &allowed.libraries {
        policy.insert(EVERY_LIBRARY.to_owned(), false.into());
        policy.insert(CHOSEN_LIBRARIES.to_owned(), libraries.clone().into());
    }
    if let Some(limit) = allowed.age_limit {
        policy.insert(AGE_LIMIT.to_owned(), limit.into());
    }
    if let Some(unrated) = allowed.unrated {
        let kinds = match unrated {
            Unrated::HeldBack => UNRATED_KINDS.to_vec(),
            Unrated::LetThrough => Vec::new(),
        };
        policy.insert(UNRATED.to_owned(), kinds.into());
    }
}

const DISABLED: &str = "IsDisabled";

const WRONG_SO_FAR: &str = "InvalidLoginAttemptCount";

const WRONG_BEFORE_LOCKOUT: &str = "LoginAttemptsBeforeLockout";

const WRONG_ALLOWED: i64 = 5;
