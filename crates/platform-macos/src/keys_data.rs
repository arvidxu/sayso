//! Key codes, event flags and mouse button numbers fixed by the system: a reference table, not
//! configuration.

/// A virtual key code: a key's position on the keyboard, the same for every keyboard layout.
pub type VirtualKey = u16;

pub mod vk {
    use super::VirtualKey;

    pub const RETURN: VirtualKey = 0x24;
    pub const TAB: VirtualKey = 0x30;
    pub const SPACE: VirtualKey = 0x31;
    pub const BACKSPACE: VirtualKey = 0x33;
    pub const ESCAPE: VirtualKey = 0x35;
    pub const RIGHT_COMMAND: VirtualKey = 0x36;
    pub const COMMAND: VirtualKey = 0x37;
    pub const SHIFT: VirtualKey = 0x38;
    pub const CAPS_LOCK: VirtualKey = 0x39;
    pub const OPTION: VirtualKey = 0x3A;
    pub const CONTROL: VirtualKey = 0x3B;
    pub const RIGHT_SHIFT: VirtualKey = 0x3C;
    pub const RIGHT_OPTION: VirtualKey = 0x3D;
    pub const RIGHT_CONTROL: VirtualKey = 0x3E;
    pub const FUNCTION: VirtualKey = 0x3F;
    pub const ANSI_V: VirtualKey = 0x09;
    pub const HOME: VirtualKey = 0x73;
    pub const PAGE_UP: VirtualKey = 0x74;
    pub const FORWARD_DELETE: VirtualKey = 0x75;
    pub const END: VirtualKey = 0x77;
    pub const PAGE_DOWN: VirtualKey = 0x79;
    pub const LEFT_ARROW: VirtualKey = 0x7B;
    pub const RIGHT_ARROW: VirtualKey = 0x7C;
    pub const DOWN_ARROW: VirtualKey = 0x7D;
    pub const UP_ARROW: VirtualKey = 0x7E;
}

pub mod flags {
    /// Caps Lock is on. It reports the lock, not whether the key is held.
    pub const CAPS_LOCK: u64 = 0x0001_0000;
    pub const SHIFT: u64 = 0x0002_0000;
    pub const CONTROL: u64 = 0x0004_0000;
    pub const OPTION: u64 = 0x0008_0000;
    pub const COMMAND: u64 = 0x0010_0000;
    /// Fn. The system also sets it on arrow, function and navigation keys whether or not Fn is
    /// held, so it says something only on an event from the Fn key itself.
    pub const FN: u64 = 0x0080_0000;

    // Which side's key is down. Hardware events carry them; an event made by a program may not.
    pub const LEFT_CONTROL: u64 = 0x0000_0001;
    pub const LEFT_SHIFT: u64 = 0x0000_0002;
    pub const RIGHT_SHIFT: u64 = 0x0000_0004;
    pub const LEFT_COMMAND: u64 = 0x0000_0008;
    pub const RIGHT_COMMAND: u64 = 0x0000_0010;
    pub const LEFT_OPTION: u64 = 0x0000_0020;
    pub const RIGHT_OPTION: u64 = 0x0000_0040;
    pub const RIGHT_CONTROL: u64 = 0x0000_2000;
}

/// The mouse button numbers of the two side buttons.
pub mod mouse {
    pub const SIDE_BACK: i64 = 3;
    pub const SIDE_FORWARD: i64 = 4;
}

/// What translating a key through the keyboard layout data takes.
#[cfg(target_os = "macos")]
pub(crate) mod layout {
    use super::VirtualKey;

    /// The highest key code a layout maps.
    pub const LAST_KEY: VirtualKey = 0x7F;
    /// The action of a key going down.
    pub const KEY_DOWN: u16 = 0;
    /// Command held, in the shifted-down form the translation takes.
    pub const COMMAND_STATE: u32 = 0x01;
    /// Leaves dead keys out, so each key yields its own character at once.
    pub const NO_DEAD_KEYS: u32 = 0x01;
    /// Room for what one key yields; layouts can map a key to a short string.
    pub const MAX_UNITS: usize = 4;
}

pub(crate) fn character_key(ch: char) -> Option<VirtualKey> {
    Some(match ch {
        'a' => 0x00,
        's' => 0x01,
        'd' => 0x02,
        'f' => 0x03,
        'h' => 0x04,
        'g' => 0x05,
        'z' => 0x06,
        'x' => 0x07,
        'c' => 0x08,
        'v' => vk::ANSI_V,
        'b' => 0x0B,
        'q' => 0x0C,
        'w' => 0x0D,
        'e' => 0x0E,
        'r' => 0x0F,
        'y' => 0x10,
        't' => 0x11,
        '1' => 0x12,
        '2' => 0x13,
        '3' => 0x14,
        '4' => 0x15,
        '6' => 0x16,
        '5' => 0x17,
        '=' => 0x18,
        '9' => 0x19,
        '7' => 0x1A,
        '-' => 0x1B,
        '8' => 0x1C,
        '0' => 0x1D,
        ']' => 0x1E,
        'o' => 0x1F,
        'u' => 0x20,
        '[' => 0x21,
        'i' => 0x22,
        'p' => 0x23,
        'l' => 0x25,
        'j' => 0x26,
        '\'' => 0x27,
        'k' => 0x28,
        ';' => 0x29,
        '\\' => 0x2A,
        ',' => 0x2B,
        '/' => 0x2C,
        'n' => 0x2D,
        'm' => 0x2E,
        '.' => 0x2F,
        '`' => 0x32,
        _ => return None,
    })
}

/// F1 to F20, in order.
const FUNCTION_KEYS: [VirtualKey; 20] = [
    0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D, 0x67, 0x6F, 0x69, 0x6B, 0x71, 0x6A,
    0x40, 0x4F, 0x50, 0x5A,
];

pub(crate) fn function_key(number: u8) -> Option<VirtualKey> {
    FUNCTION_KEYS
        .get(usize::from(number).checked_sub(1)?)
        .copied()
}

pub(crate) fn is_function_key(key: VirtualKey) -> bool {
    FUNCTION_KEYS.contains(&key)
}
