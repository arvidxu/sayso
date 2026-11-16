//! Already read out of the system event, so the matcher can be tested without the system.

use crate::keys::VirtualKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    /// `repeat` is set when the key is being held and the system repeats it.
    KeyDown {
        key: VirtualKey,
        repeat: bool,
    },
    KeyUp {
        key: VirtualKey,
    },
    /// A modifier key went down or up, or Caps Lock was pressed. The flags say which.
    FlagsChanged {
        key: VirtualKey,
    },
    /// A mouse button went down: 0 is the main button, 1 the secondary one, 2 the middle one,
    /// and the side buttons follow.
    MouseDown {
        button: i64,
    },
    MouseUp {
        button: i64,
    },
    MouseDragged {
        button: i64,
    },
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub kind: InputKind,
    /// See [`crate::keys::flags`].
    pub flags: u64,
    /// Milliseconds on this package's clock, which only moves forward.
    pub timestamp_ms: u64,
}

impl InputEvent {
    pub fn new(kind: InputKind, flags: u64, timestamp_ms: u64) -> Self {
        Self {
            kind,
            flags,
            timestamp_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    PassThrough,
    Swallow,
}
