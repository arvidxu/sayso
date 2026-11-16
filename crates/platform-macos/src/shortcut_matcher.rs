//! Decides from keyboard and mouse events when a shortcut goes down, comes up or is
//! interrupted, and which events to keep from the application.

use dictation_types::{
    AppError, AppResult, HotkeyBinding, ShortcutEvent, ShortcutId, ShortcutState,
};

use crate::clock::now_ms;
use crate::input::{Disposition, InputEvent, InputKind};
use crate::keys::{VirtualKey, flags, modifier_of_key, vk};
pub use crate::keys_down::KeyState;
use crate::keys_down::KeysDown;
use crate::trigger::{Active, Trigger};

#[derive(Default)]
pub struct ShortcutMatcher {
    bindings: Vec<(HotkeyBinding, Trigger)>,
    down: KeysDown,
    active: Option<Active>,
    /// Events from changing the bindings, for the caller to send.
    pending: Vec<ShortcutEvent>,
}

impl ShortcutMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// A matcher that asks `key_state` whether keys it believes are down really are, before
    /// one of them keeps a shortcut from going down. Without it, a key whose release was never
    /// seen blocks lone modifiers, Caps Lock and mouse buttons until that key is pressed again.
    pub fn with_key_state(key_state: Box<dyn KeyState>) -> Self {
        Self {
            down: KeysDown::with_key_state(key_state),
            ..Self::default()
        }
    }

    /// Events caused by changing the bindings since the last call: a shortcut removed or
    /// changed while it was down is interrupted.
    pub fn take_pending(&mut self) -> Vec<ShortcutEvent> {
        std::mem::take(&mut self.pending)
    }

    /// Replaces one with the same ID. Refuses a binding the system cannot
    /// watch, or one on the same keys as another (a key reached through Fn is the same with or
    /// without Fn). Replacing it with one on the same keys keeps a press going.
    pub fn register(&mut self, binding: HotkeyBinding) -> AppResult<()> {
        let trigger = Trigger::of(&binding)?;
        if let Some((other, _)) = self
            .bindings
            .iter()
            .find(|(other, existing)| other.id != binding.id && existing.same_keys(&trigger))
        {
            return Err(AppError::already_exists(format!(
                "shortcut '{}' already uses these keys",
                other.id
            )));
        }
        if let Some(slot) = self
            .bindings
            .iter_mut()
            .find(|(existing, existing_trigger)| {
                existing.id == binding.id && *existing_trigger == trigger
            })
        {
            slot.0 = binding;
            return Ok(());
        }
        self.unregister(&binding.id);
        self.bindings.push((binding, trigger));
        Ok(())
    }

    /// A press of it in progress is
    /// interrupted; see [`Self::take_pending`].
    pub fn unregister(&mut self, id: &ShortcutId) -> bool {
        let before = self.bindings.len();
        self.bindings.retain(|(binding, _)| &binding.id != id);
        if self.active.as_ref().is_some_and(|active| active.id() == id) {
            self.interrupt_pending();
        }
        self.bindings.len() != before
    }

    /// Interrupts a press in progress.
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.interrupt_pending();
    }

    fn interrupt_pending(&mut self) {
        let mut events = Vec::new();
        self.interrupt_active(now_ms(), &mut events);
        self.pending.extend(events);
    }

    pub fn bindings(&self) -> Vec<HotkeyBinding> {
        self.bindings
            .iter()
            .map(|(binding, _)| binding.clone())
            .collect()
    }

    /// Ends a press in progress after events may have been missed and forgets what is down, so the
    /// first press after the gap is seen. Keys kept from the application stay kept until released.
    pub fn interrupt(&mut self, timestamp_ms: u64) -> Option<ShortcutEvent> {
        self.down.forget();
        self.active
            .take()
            .map(|active| event(active.id(), ShortcutState::Interrupted, timestamp_ms))
    }

    pub fn handle(&mut self, input: InputEvent) -> (Vec<ShortcutEvent>, Disposition) {
        let mut events = Vec::new();
        let disposition = match input.kind {
            InputKind::KeyDown { key, repeat } => self.key_down(key, repeat, input, &mut events),
            InputKind::KeyUp { key } => self.key_up(key, input, &mut events),
            InputKind::FlagsChanged { key } if key == vk::CAPS_LOCK => {
                self.caps_lock(input, &mut events);
                Disposition::PassThrough
            }
            InputKind::FlagsChanged { key } => {
                self.modifier_changed(key, input, &mut events);
                Disposition::PassThrough
            }
            InputKind::MouseDown { button } => self.mouse_down(button, input, &mut events),
            InputKind::MouseUp { button } => self.mouse_up(button, input, &mut events),
            InputKind::MouseDragged { button } if self.down.swallowed_buttons.contains(&button) => {
                Disposition::Swallow
            }
            InputKind::MouseDragged { button } => {
                // A drag says the button is down even when its press was not seen.
                self.down.buttons.insert(button);
                self.interrupt_lone_modifier(input.timestamp_ms, &mut events);
                Disposition::PassThrough
            }
            InputKind::Scroll => {
                self.interrupt_lone_modifier(input.timestamp_ms, &mut events);
                Disposition::PassThrough
            }
        };
        if !matches!(input.kind, InputKind::FlagsChanged { key } if key == vk::CAPS_LOCK) {
            self.down.caps_lock_on = Some(input.flags & flags::CAPS_LOCK != 0);
        }
        (events, disposition)
    }

    fn key_down(
        &mut self,
        key: VirtualKey,
        repeat: bool,
        input: InputEvent,
        events: &mut Vec<ShortcutEvent>,
    ) -> Disposition {
        if self.down.swallowed_keys.contains(&key) {
            if repeat {
                return Disposition::Swallow;
            }
            // A fresh press of a key whose release was missed.
            self.down.swallowed_keys.remove(&key);
        }
        if repeat {
            return Disposition::PassThrough;
        }
        self.down.sync_held(input);
        self.interrupt_active(input.timestamp_ms, events);
        let found = self
            .bindings
            .iter()
            .find_map(|(binding, trigger)| match trigger {
                Trigger::Combination {
                    key: wanted,
                    modifiers,
                } if *wanted == key && self.down.holding_exactly(key, modifiers) => {
                    Some((binding.id.clone(), modifiers.clone()))
                }
                _ => None,
            });
        match found {
            Some((id, modifiers)) => {
                events.push(event(&id, ShortcutState::Pressed, input.timestamp_ms));
                self.active = Some(Active::Combination { id, key, modifiers });
                self.down.swallowed_keys.insert(key);
                Disposition::Swallow
            }
            None => {
                self.down.others.insert(key);
                Disposition::PassThrough
            }
        }
    }

    fn key_up(
        &mut self,
        key: VirtualKey,
        input: InputEvent,
        events: &mut Vec<ShortcutEvent>,
    ) -> Disposition {
        self.down.others.remove(&key);
        if !self.down.swallowed_keys.remove(&key) {
            return Disposition::PassThrough;
        }
        if matches!(&self.active, Some(Active::Combination { key: down, .. }) if *down == key) {
            self.release_active(input.timestamp_ms, events);
        }
        Disposition::Swallow
    }

    fn caps_lock(&mut self, input: InputEvent, events: &mut Vec<ShortcutEvent>) {
        let on = input.flags & flags::CAPS_LOCK != 0;
        // The system may report the key going up as well; only a change of the lock is a press.
        let pressed = self.down.caps_lock_on != Some(on);
        self.down.caps_lock_on = Some(on);
        if !pressed {
            return;
        }
        self.down.sync_held(input);
        self.interrupt_active(input.timestamp_ms, events);
        if !self.down.held.is_empty() || !self.down.nothing_else_down() {
            return;
        }
        if let Some(id) = self.bound(|trigger| *trigger == Trigger::CapsLock) {
            events.push(event(&id, ShortcutState::Pressed, input.timestamp_ms));
            events.push(event(&id, ShortcutState::Released, input.timestamp_ms));
        }
    }

    fn modifier_changed(
        &mut self,
        key: VirtualKey,
        input: InputEvent,
        events: &mut Vec<ShortcutEvent>,
    ) {
        let before = self.down.held.clone();
        self.down.sync_held(input);
        let went_down = self.down.held.difference(&before).next().is_some();
        let still_held = match &self.active {
            None => None,
            Some(Active::Combination { modifiers, .. }) => {
                Some(modifiers.is_subset(&self.down.held))
            }
            Some(Active::Modifier { modifier, .. }) => Some(self.down.held.contains(modifier)),
            Some(Active::MouseButton { .. }) => Some(true),
        };
        match still_held {
            Some(false) => self.release_active(input.timestamp_ms, events),
            Some(true) if went_down => self.interrupt_active(input.timestamp_ms, events),
            Some(true) => {}
            None => {
                let Some(modifier) = modifier_of_key(key) else {
                    return;
                };
                let alone = went_down
                    && self.down.held.len() == 1
                    && self.down.held.contains(&modifier)
                    && self.down.nothing_else_down();
                if !alone {
                    return;
                }
                if let Some(id) = self.bound(|trigger| *trigger == Trigger::Modifier(modifier)) {
                    events.push(event(&id, ShortcutState::Pressed, input.timestamp_ms));
                    self.active = Some(Active::Modifier { id, modifier });
                }
            }
        }
    }

    fn mouse_down(
        &mut self,
        button: i64,
        input: InputEvent,
        events: &mut Vec<ShortcutEvent>,
    ) -> Disposition {
        self.down.sync_held(input);
        // A fresh press of this button means its earlier release was missed, whether or not it
        // is still a shortcut, so its release and drags are no longer kept from the application.
        self.down.swallowed_buttons.remove(&button);
        self.down.buttons.remove(&button);
        let Some(id) = self.bound(|trigger| *trigger == Trigger::MouseButton(button)) else {
            // A click with a modifier held, such as Shift-click, uses the modifier for
            // something else.
            self.interrupt_lone_modifier(input.timestamp_ms, events);
            self.down.buttons.insert(button);
            return Disposition::PassThrough;
        };
        self.interrupt_active(input.timestamp_ms, events);
        if !self.down.held.is_empty() || !self.down.nothing_else_down() {
            self.down.buttons.insert(button);
            return Disposition::PassThrough;
        }
        events.push(event(&id, ShortcutState::Pressed, input.timestamp_ms));
        self.active = Some(Active::MouseButton { id, button });
        self.down.swallowed_buttons.insert(button);
        Disposition::Swallow
    }

    fn mouse_up(
        &mut self,
        button: i64,
        input: InputEvent,
        events: &mut Vec<ShortcutEvent>,
    ) -> Disposition {
        if !self.down.swallowed_buttons.remove(&button) {
            self.down.buttons.remove(&button);
            return Disposition::PassThrough;
        }
        if matches!(&self.active, Some(Active::MouseButton { button: down, .. }) if *down == button)
        {
            self.release_active(input.timestamp_ms, events);
        }
        Disposition::Swallow
    }

    fn bound(&self, wanted: impl Fn(&Trigger) -> bool) -> Option<ShortcutId> {
        self.bindings
            .iter()
            .find(|(_, trigger)| wanted(trigger))
            .map(|(binding, _)| binding.id.clone())
    }

    fn release_active(&mut self, timestamp_ms: u64, events: &mut Vec<ShortcutEvent>) {
        if let Some(active) = self.active.take() {
            events.push(event(active.id(), ShortcutState::Released, timestamp_ms));
        }
    }

    /// Interrupts a modifier down on its own, which a click, a drag or a scroll uses for
    /// something else, as in Shift-click or Control-scroll. A combination or a mouse button stays down.
    fn interrupt_lone_modifier(&mut self, timestamp_ms: u64, events: &mut Vec<ShortcutEvent>) {
        if matches!(self.active, Some(Active::Modifier { .. })) {
            self.interrupt_active(timestamp_ms, events);
        }
    }

    fn interrupt_active(&mut self, timestamp_ms: u64, events: &mut Vec<ShortcutEvent>) {
        if let Some(active) = self.active.take() {
            events.push(event(active.id(), ShortcutState::Interrupted, timestamp_ms));
        }
    }
}

fn event(id: &ShortcutId, state: ShortcutState, timestamp_ms: u64) -> ShortcutEvent {
    ShortcutEvent {
        binding_id: id.clone(),
        state,
        timestamp_ms,
    }
}
