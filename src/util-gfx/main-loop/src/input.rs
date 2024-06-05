use bevy_ecs::system::Resource;
use newtypes::{define_index, IndexVec};
use rustc_hash::FxHashMap;
use typed_glam::glam::DVec2;
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, WindowEvent},
    keyboard::{Key, PhysicalKey},
    platform::modifier_supplement::KeyEventExtModifierSupplement,
    window::WindowId,
};

// === InputManager === //

#[derive(Debug, Resource, Default)]
pub struct InputManager {
    windows: FxHashMap<WindowId, WindowDeviceState>,
    mouse_delta: DVec2,
}

impl InputManager {
    pub fn process_window_event(&mut self, window: WindowId, event: &WindowEvent) {
        if let WindowEvent::Destroyed = event {
            self.windows.remove(&window);
        } else {
            self.windows.entry(window).or_default().process(event);
        }
    }

    pub fn process_device_event(&mut self, _device: DeviceId, event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.mouse_delta += DVec2::from(*delta)
        }
    }

    pub fn end_tick(&mut self) {
        self.mouse_delta = DVec2::ZERO;

        for win in self.windows.values_mut() {
            win.end_tick();
        }
    }

    pub fn window(&self, window: WindowId) -> InputManagerWindow<'_> {
        InputManagerWindow(self.windows.get(&window))
    }

    pub fn mouse_delta(&self) -> DVec2 {
        self.mouse_delta
    }
}

#[derive(Debug, Default)]
struct WindowDeviceState {
    agg_keyboard: KeyboardDeviceState,
    agg_mouse: MouseDeviceState,
    agg_mouse_pos: Option<PhysicalPosition<f64>>,
    keyboards: FxHashMap<DeviceId, KeyboardDeviceState>,
    mice: FxHashMap<DeviceId, MouseDeviceState>,
}

impl WindowDeviceState {
    fn process(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                device_id, event, ..
            } => {
                self.agg_keyboard.process(event);
                self.keyboards.entry(*device_id).or_default().process(event);
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                self.agg_mouse.process(*button, *state);
                self.mice
                    .entry(*device_id)
                    .or_default()
                    .process(*button, *state);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.agg_mouse_pos = Some(*position);
            }
            _ => {}
        }
    }

    fn end_tick(&mut self) {
        self.keyboards.retain(|_, v| v.end_tick());
        self.mice.retain(|_, v| v.end_tick());
    }
}

#[derive(Debug, Default)]
struct KeyboardDeviceState {
    logical_keys: FxHashMap<Key, BoolAction>,
    physical_keys: FxHashMap<PhysicalKey, BoolAction>,
}

impl KeyboardDeviceState {
    fn process(&mut self, event: &KeyEvent) {
        let is_pressed = event.state.is_pressed();

        self.logical_keys
            .entry(event.key_without_modifiers())
            .or_default()
            .set_state(is_pressed);

        self.physical_keys
            .entry(event.physical_key)
            .or_default()
            .set_state(is_pressed);
    }

    fn end_tick(&mut self) -> bool {
        self.logical_keys.retain(|_, v| v.end_tick());
        self.physical_keys.retain(|_, v| v.end_tick());

        !self.logical_keys.is_empty() || !self.physical_keys.is_empty()
    }

    fn logical_key(&self, key: Key) -> BoolAction {
        self.logical_keys.get(&key).copied().unwrap_or_default()
    }

    fn physical_key(&self, key: PhysicalKey) -> BoolAction {
        self.physical_keys.get(&key).copied().unwrap_or_default()
    }
}

#[derive(Debug, Default)]
struct MouseDeviceState {
    buttons: IndexVec<MouseButtonIndex, BoolAction>,
}

impl MouseDeviceState {
    fn process(&mut self, button: MouseButton, state: ElementState) {
        let is_pressed = state.is_pressed();
        let button = MouseButtonIndex::from(button);

        self.buttons.entry(button).set_state(is_pressed);
    }

    fn end_tick(&mut self) -> bool {
        let mut max_kept = 0;

        for (i, button) in self.buttons.raw.iter_mut().enumerate() {
            if button.end_tick() {
                max_kept = i;
            }
        }

        self.buttons.raw.truncate(max_kept);

        max_kept > 0
    }

    fn button(&self, button: MouseButton) -> BoolAction {
        self.buttons[MouseButtonIndex::from(button)]
    }
}

define_index! {
    struct MouseButtonIndex: u32;
}

impl From<MouseButton> for MouseButtonIndex {
    fn from(button: MouseButton) -> Self {
        MouseButtonIndex(match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Back => 3,
            MouseButton::Forward => 4,
            MouseButton::Other(i) => 5 + i as u32,
        })
    }
}

impl From<MouseButtonIndex> for MouseButton {
    fn from(button: MouseButtonIndex) -> Self {
        match button.0 {
            0 => MouseButton::Left,
            1 => MouseButton::Right,
            2 => MouseButton::Middle,
            3 => MouseButton::Back,
            4 => MouseButton::Forward,
            i => MouseButton::Other((i - 5) as u16),
        }
    }
}

// === InputManager Facades === //

#[derive(Debug, Copy, Clone)]
pub struct InputManagerWindow<'a>(Option<&'a WindowDeviceState>);

impl<'a> InputManagerWindow<'a> {
    pub fn physical_key(self, key: PhysicalKey) -> BoolAction {
        self.0
            .map_or(BoolAction::default(), |v| v.agg_keyboard.physical_key(key))
    }

    pub fn logical_key(self, key: Key) -> BoolAction {
        self.0
            .map_or(BoolAction::default(), |v| v.agg_keyboard.logical_key(key))
    }

    pub fn button(self, button: MouseButton) -> BoolAction {
        self.0
            .map_or(BoolAction::default(), |v| v.agg_mouse.button(button))
    }

    pub fn mouse_pos(self) -> Option<PhysicalPosition<f64>> {
        self.0.and_then(|v| v.agg_mouse_pos)
    }

    pub fn keyboard(self, device: DeviceId) -> InputManagerKeyboard<'a> {
        InputManagerKeyboard(self.0.and_then(|v| v.keyboards.get(&device)))
    }

    pub fn mouse(self, device: DeviceId) -> InputManagerMouse<'a> {
        InputManagerMouse(self.0.and_then(|v| v.mice.get(&device)))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct InputManagerKeyboard<'a>(Option<&'a KeyboardDeviceState>);

impl InputManagerKeyboard<'_> {
    pub fn physical_key(self, key: PhysicalKey) -> BoolAction {
        self.0
            .map_or(BoolAction::default(), |v| v.physical_key(key))
    }

    pub fn logical_key(self, key: Key) -> BoolAction {
        self.0.map_or(BoolAction::default(), |v| v.logical_key(key))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct InputManagerMouse<'a>(Option<&'a MouseDeviceState>);

impl InputManagerMouse<'_> {
    pub fn button(self, button: MouseButton) -> BoolAction {
        self.0.map_or(BoolAction::default(), |v| v.button(button))
    }
}

// === BoolAction === //

#[derive(Debug, Copy, Clone, Default)]
pub struct BoolAction {
    changes: u8,
    state: bool,
}

impl BoolAction {
    /// Updates the state of the action.
    pub fn set_state(&mut self, state: bool) {
        if self.state != state {
            self.state = state;

            if self.changes < u8::MAX - 1 {
                self.changes += 1;
            }
        }
    }

    /// Gets the current state of the action.
    pub fn state(&self) -> bool {
        self.state
    }

    /// Gets the state of the action when the last tick ended.
    pub fn original_state(&self) -> bool {
        if self.changes % 2 == 0 {
            self.state
        } else {
            !self.state
        }
    }

    /// Returns the number of times the action's state changed since the last tick ended.
    pub fn times_changed(&self) -> u8 {
        self.changes
    }

    /// Returns the number of times the action transitioned to a given state since the last tick ended.
    pub fn times_trans(&self, state: bool) -> u8 {
        if self.state == state {
            (self.changes + 1) / 2
        } else {
            self.changes / 2
        }
    }

    /// Returns the number of times the action was pressed since the last tick ended.
    pub fn times_pressed(&self) -> u8 {
        self.times_trans(true)
    }

    /// Returns The number of times the action was released since the last tick ended.
    pub fn times_released(&self) -> u8 {
        self.times_trans(false)
    }

    /// Returns whether the button transitioned to a given state since the last tick ended.
    pub fn recently_became(&self, state: bool) -> bool {
        if self.changes == 1 {
            self.state == state
        } else {
            self.changes != 0
        }
    }

    /// Returns whether the button became pressed since the last tick ended.
    pub fn recently_pressed(&self) -> bool {
        self.recently_became(true)
    }

    /// Returns whether the button became released since the last tick ended.
    pub fn recently_released(&self) -> bool {
        self.recently_became(false)
    }

    /// Signals the end of the tick. Returns true if the resulting action is identical to `Default`.
    pub fn end_tick(&mut self) -> bool {
        self.changes = 0;
        self.state
    }
}
