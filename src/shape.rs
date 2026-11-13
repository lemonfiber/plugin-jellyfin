const DEVICE_SHORTEST: usize = 8;

const DEVICE_LONGEST: usize = 64;

const KEY_LONGEST: usize = 128;

const HEX_DIGITS: usize = 32;

const PROFILE_LONGEST: usize = 32;

/// Whether `id` is shaped like an id Jellyfin files an item or an account under:
/// thirty-two hex digits, with or without the four dashes, and not all of them zero, the
/// empty id Jellyfin answers its root folder for.
#[must_use]
pub fn an_id(id: &str) -> bool {
    let digits: Vec<char> = id.chars().filter(|letter| *letter != '-').collect();
    let dashed = id.len() == 36
        && id
            .char_indices()
            .all(|(at, letter)| (letter == '-') == matches!(at, 8 | 13 | 18 | 23));
    (id.len() == 32 || dashed)
        && digits.len() == 32
        && digits.iter().all(char::is_ascii_hexdigit)
        && digits.iter().any(|digit| *digit != '0')
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

/// Whether `text` is thirty-two lowercase hex digits, as Jellyfin writes an id with no
/// dashes.
#[must_use]
pub fn hex(text: &str) -> bool {
    text.len() == HEX_DIGITS
        && text
            .bytes()
            .all(|digit| digit.is_ascii_digit() || (b'a'..=b'f').contains(&digit))
}

/// Whether `id` is shaped like the id Jellyfin opens a live stream under: three ids of
/// thirty-two lowercase hex digits joined by `_`, or, from a tuner that names a profile,
/// the first two of them, the profile in lowercase letters and digits, and two more.
#[must_use]
pub fn a_live_stream(id: &str) -> bool {
    let parts: Vec<&str> = id.split('_').collect();
    match parts.as_slice() {
        [first, second, source] => hex(first) && hex(second) && hex(source),
        [first, second, profile, channel, address] => {
            hex(first)
                && hex(second)
                && (1..=PROFILE_LONGEST).contains(&profile.len())
                && profile
                    .bytes()
                    .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit())
                && hex(channel)
                && hex(address)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
