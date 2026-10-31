use super::{a_device, a_key, an_id};

#[test]
fn an_id_is_thirty_two_hex_digits_with_or_without_its_dashes() {
    assert!(an_id("4f53cda18c2baa0c0354bb5f9a3ecbe5"));
    assert!(an_id("4f53cda1-8c2b-aa0c-0354-bb5f9a3ecbe5"));
    for refused in [
        "",
        "4f53cda18c2baa0c0354bb5f9a3ecbe",
        "4f53cda18c2baa0c0354bb5f9a3ecbe5a",
        "4f53cda1-8c2baa0c-0354-bb5f9a3ecbe5",
        "../Users/4f53cda18c2baa0c0354bb5f9",
        "4f53cda18c2baa0c0354bb5f9a3ecbeg",
        "4f53cda18c2baa0c0354bb5f9a3ecb?5",
    ] {
        assert!(!an_id(refused), "{refused:?} was taken for an id");
    }
}

#[test]
fn a_device_is_eight_to_sixty_four_letters_digits_and_dashes() {
    assert!(a_device("phone-0001"));
    assert!(a_device(&"a".repeat(64)));
    for refused in ["short", "", "has a space", "quote\"close", &"a".repeat(65)] {
        assert!(!a_device(refused), "{refused:?} was taken for a device");
    }
}

#[test]
fn a_key_is_letters_and_digits_and_never_a_path() {
    assert!(a_key("0f3a9c2b"));
    for refused in ["", "a/b", "a?b=c", "../keys", &"a".repeat(129)] {
        assert!(!a_key(refused), "{refused:?} was taken for a key");
    }
}
