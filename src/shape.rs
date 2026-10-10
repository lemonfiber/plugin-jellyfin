const DEVICE_SHORTEST: usize = 8;

const DEVICE_LONGEST: usize = 64;

const KEY_LONGEST: usize = 128;

/// Whether `id` is shaped like an id Jellyfin files an item or an account under:
/// thirty-two hex digits, with or without the four dashes.
#[must_use]
pub fn an_id(id: &str) -> bool {
    let digits: Vec<char> = id.chars().filter(|letter| *letter != '-').collect();
    let dashed = id.len() == 36
        && id
            .char_indices()
            .all(|(at, letter)| (letter == '-') == matches!(at, 8 | 13 | 18 | 23));
    (id.len() == 32 || dashed) && digits.len() == 32 && digits.iter().all(char::is_ascii_hexdigit)
}

/// A value held to the shape of an id Jellyfin files an item or an account under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Id<'a>(&'a str);

impl<'a> Id<'a> {
    /// `text`, where it is shaped like an id.
    #[must_use]
    pub fn of(text: &'a str) -> Option<Self> {
        an_id(text).then_some(Self(text))
    }
}

impl std::fmt::Display for Id<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Whether `device` is shaped like the id a player keeps for the device it plays on: eight
/// to sixty-four letters, digits and dashes.
#[must_use]
pub fn a_device(device: &str) -> bool {
    (DEVICE_SHORTEST..=DEVICE_LONGEST).contains(&device.len())
        && device
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '-')
}

/// Whether `key` is shaped like a key Jellyfin mints: letters and digits, and not empty.
#[must_use]
pub fn a_key(key: &str) -> bool {
    (1..=KEY_LONGEST).contains(&key.len())
        && key.chars().all(|letter| letter.is_ascii_alphanumeric())
}

/// Whether `media_type` names an image type other than SVG.
#[must_use]
pub fn a_picture(media_type: &str) -> bool {
    let essence = media_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    essence
        .strip_prefix("image/")
        .is_some_and(|subtype| !subtype.is_empty() && !subtype.starts_with("svg"))
}

#[cfg(test)]
mod tests;
