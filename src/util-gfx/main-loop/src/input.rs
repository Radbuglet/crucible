use winit::{
    event::{DeviceId, KeyEvent},
    platform::modifier_supplement::KeyEventExtModifierSupplement,
};

/// Tracks the state of various stateful devices such as keyboards and mice. This system may also
/// eventually track other forms of input such as controllers. This service also tracks one-off events
/// such that their handling can be deferred.
pub struct InputManager {}

impl InputManager {
    pub fn process(&mut self, device: DeviceId, event: &KeyEvent) {
        &event.location;
        &event.physical_key;
        &event.logical_key;
        event.key_without_modifiers();
        &event.text;
        &event.repeat;
        &event.state;
    }
}
