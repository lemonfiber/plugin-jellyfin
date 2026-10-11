use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::Failure;

use super::Jellyfin;

pub(super) struct Listed {
    /// Where the document is read and written.
    pub document: &'static str,
    /// The field holding the list.
    pub field: &'static str,
    /// What the list is, for a refusal.
    pub what: &'static str,
}

impl Listed {
    pub(super) async fn read(&self, jellyfin: &Jellyfin) -> Result<Vec<String>, Failure> {
        Ok(self.entries(&self.document(jellyfin).await?))
    }

    pub(super) async fn only(&self, jellyfin: &Jellyfin, entry: &str) -> Result<(), Failure> {
        let mut document = self.document(jellyfin).await?;
        let Some(fields) = document.as_object_mut() else {
            return Err(jellyfin.endpoint.refused(&format!(
                "the document holding {} is not an object",
                self.what
            )));
        };
        fields.insert(self.field.to_owned(), serde_json::json!([entry]));
        let response = jellyfin
            .as_admin(Method::Post, self.document, Some(document.to_string()))
            .await?;
        jellyfin.endpoint.expect_success(&response)?;
        let kept = self.read(jellyfin).await?;
        if kept != [entry] {
            return Err(jellyfin.endpoint.refused(&format!(
                "{} was written as {entry} and reads back as {kept:?}",
                self.what
            )));
        }
        Ok(())
    }

    async fn document(&self, jellyfin: &Jellyfin) -> Result<serde_json::Value, Failure> {
        document(jellyfin, self.document, self.what).await
    }

    fn entries(&self, document: &serde_json::Value) -> Vec<String> {
        document
            .get(self.field)
            .and_then(serde_json::Value::as_array)
            .map(|listed| {
                listed
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub(super) const CONFIGURATION: &str = "/System/Configuration";

pub(super) async fn document(
    jellyfin: &Jellyfin,
    path: &str,
    what: &str,
) -> Result<serde_json::Value, Failure> {
    let response = jellyfin.as_admin(Method::Get, path, None).await?;
    jellyfin.endpoint.decode(
        &response,
        &format!("the document holding {what} could not be read"),
    )
}
