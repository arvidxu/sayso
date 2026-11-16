use platform_macos::keys::{paste_key_among, vk};

fn yields(pairs: &[(u16, &str)]) -> Vec<(u16, String)> {
    pairs
        .iter()
        .map(|(key, text)| (*key, (*text).to_owned()))
        .collect()
}

#[test]
fn the_key_that_yields_v_with_command_pastes() {
    // Dvorak keeps its own positions with Command held: V sits where US has the period.
    let dvorak = yields(&[(0x09, "k"), (0x2F, "v"), (0x00, "a")]);
    assert_eq!(paste_key_among(dvorak), 0x2F);
}

#[test]
fn a_layout_that_falls_back_to_us_positions_under_command_pastes_on_the_us_key() {
    let us_under_command = yields(&[(0x08, "c"), (vk::ANSI_V, "v"), (0x0B, "b")]);
    assert_eq!(paste_key_among(us_under_command), vk::ANSI_V);
}

#[test]
fn several_keys_yielding_v_prefer_the_us_key_then_the_lowest() {
    let with_us = yields(&[(0x2F, "v"), (vk::ANSI_V, "v")]);
    assert_eq!(paste_key_among(with_us), vk::ANSI_V);
    let without_us = yields(&[(0x2F, "v"), (0x20, "v")]);
    assert_eq!(paste_key_among(without_us), 0x20);
}

#[test]
fn without_any_key_yielding_v_the_us_key_is_used() {
    assert_eq!(paste_key_among(Vec::new()), vk::ANSI_V);
    // A capital or a longer string is not the plain letter.
    let none = yields(&[(0x10, "V"), (0x11, "vv"), (0x12, "м")]);
    assert_eq!(paste_key_among(none), vk::ANSI_V);
}
