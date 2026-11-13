use super::{a_device, a_key, a_live_stream, an_id, hex};

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
        "00000000000000000000000000000000",
        "00000000-0000-0000-0000-000000000000",
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

#[test]
fn hex_is_thirty_two_lowercase_hex_digits_and_nothing_else() {
    assert!(hex("4f53cda18c2baa0c0354bb5f9a3ecbe5"));
    for refused in [
        "",
        "4F53CDA18C2BAA0C0354BB5F9A3ECBE5",
        "4f53cda1-8c2b-aa0c-0354-bb5f9a3ecbe5",
        "4f53cda18c2baa0c0354bb5f9a3ecbe",
        "4f53cda18c2baa0c0354bb5f9a3ecbe5a",
        "4f53cda18c2baa0c0354bb5f9a3ecbeg",
    ] {
        assert!(!hex(refused), "{refused:?} was taken for hex");
    }
}

#[test]
fn a_live_stream_is_three_ids_or_two_a_profile_and_two() {
    let id = "e2329f4997b378e64ccf8fa396deb76e";
    assert!(a_live_stream(&format!("{id}_{id}_{id}")));
    assert!(a_live_stream(&format!("{id}_{id}_native_{id}_{id}")));
    assert!(a_live_stream(&format!("{id}_{id}_internet720_{id}_{id}")));
    for refused in [
        String::new(),
        "conformance-live-stream".to_owned(),
        id.to_owned(),
        format!("{id}_{id}"),
        format!("{id}_{id}_{id}_{id}"),
        format!("{id}_{id}_{id}_"),
        format!("{id}_{id}_{}", id.to_uppercase()),
        format!("{id}_{id}__{id}_{id}"),
        format!("{id}_{id}_Native_{id}_{id}"),
        format!("{id}_{id}_na-tive_{id}_{id}"),
        format!("{id}_{id}_{}_{id}_{id}", "a".repeat(33)),
        format!("{id}_{id}_{id}&x=1"),
        format!("../{id}_{id}_{id}"),
    ] {
        assert!(
            !a_live_stream(&refused),
            "{refused:?} was taken for a live stream"
        );
    }
}
