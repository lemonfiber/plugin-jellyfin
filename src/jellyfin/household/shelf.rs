use super::super::item::{ItemResource, ItemsResource};
use super::{item_type, Jellyfin, Kind, Method};
use lemonfiber_ports::service::{Failure, Item};

pub(super) async fn holdings(
    jellyfin: &Jellyfin,
    member: Option<&str>,
    most: u32,
) -> Result<Vec<Item>, Failure> {
    if member.is_some_and(|member| !crate::shape::an_id(member)) {
        return Err(jellyfin.misshapen("an account"));
    }
    let response = jellyfin
        .as_admin(Method::Get, &asked(member, most), None)
        .await?;
    let held: ItemsResource = jellyfin
        .endpoint
        .decode(&response, "what the household holds could not be read")?;

    Ok(held.items.into_iter().map(ItemResource::item).collect())
}

fn asked(member: Option<&str>, most: u32) -> String {
    let kinds = Kind::ALL.map(item_type).join(",");
    let whose = member.map_or_else(String::new, |member| format!("/Users/{member}"));
    format!(
        "{whose}/Items?Recursive=true&IncludeItemTypes={kinds}\
         &SortBy=DateCreated&SortOrder=Descending&Limit={most}"
    )
}
