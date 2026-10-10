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

#[cfg(test)]
mod tests;
