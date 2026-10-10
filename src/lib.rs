//! lemonfiber's adapter for Jellyfin: the `media.serve` and `identity.source` contracts,
//! spoken in front of one Jellyfin server and translated to its own API.

pub mod adapter;
pub mod credential;
mod endpoint;
pub mod jellyfin;
pub mod settings;
mod shape;
pub mod web;
