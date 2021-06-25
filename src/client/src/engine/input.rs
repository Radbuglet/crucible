use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use arbre::provider::provide;
use cgmath::{Vector2, Zero};
use winit::event::{DeviceEvent, WindowEvent, KeyboardInput, VirtualKeyCode, MouseButton, ElementState};
use super::{WinitEvent, WindowPosPx};

/// Tracks keyboard & mouse input states. Users may still need to listen for events to detect certain
/// actions.
#[derive(Default)]
pub struct InputTracker {
    inner: RefCell<InputTrackerInner>,
}

struct InputTrackerInner {
    keys: HashMap<VirtualKeyCode, BoolAction>,
    mouse_buttons: HashMap<MouseButton, BoolAction>,
    mouse_pos: Option<WindowPosPx>,
    mouse_delta: Vector2<f64>,
    has_focus: bool,
}

impl Default for InputTrackerInner {
    fn default() -> Self {
        Self {
            keys: Default::default(),
            mouse_buttons: Default::default(),
            mouse_pos: None,
            mouse_delta: Vector2::zero(),
            has_focus: true,
        }
    }
}

provide! { InputTracker => Self }

impl InputTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle(&self, event: &WinitEvent) {
        fn set_state_in_map<K: Hash + Eq>(map: &mut HashMap<K, BoolAction>, key: K, value: bool) {
            let action = map.entry(key).or_insert(Default::default());
            action.set_state(value);
        }

        let mut inner = self.inner.borrow_mut();

        match event {
            // On loose focus
            WinitEvent::WindowEvent { event: WindowEvent::Focused(has_focus), .. } => {
                if !*has_focus && inner.has_focus {
                    for key in inner.keys.values_mut() {
                        key.set_state(false);
                    }

                    for button in inner.mouse_buttons.values_mut() {
                        button.set_state(false);
                    }

                    inner.mouse_pos = None;
                }

                inner.has_focus = *has_focus;
            }

            // On key update
            WinitEvent::WindowEvent { event: WindowEvent::KeyboardInput {
                    input: KeyboardInput {
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                    ..
                },
                ..
            } => {
                set_state_in_map(&mut inner.keys, *keycode, *state == ElementState::Pressed);
            }

            // On mouse button update
            WinitEvent::WindowEvent { event: WindowEvent::MouseInput { state, button, .. }, .. } => {
                set_state_in_map(
                    &mut inner.mouse_buttons,
                    *button,
                    *state == ElementState::Pressed,
                );
            }

            // On mouse move
            WinitEvent::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => {
                inner.mouse_pos = Some(*position);
            }

            // On mouse exit
            WinitEvent::WindowEvent { event: WindowEvent::CursorLeft { .. }, .. } => {
                inner.mouse_pos = None;
            }

            // On locked mouse move
            WinitEvent::DeviceEvent { event: DeviceEvent::MouseMotion { delta: (dx, dy) }, .. } => {
                inner.mouse_delta += Vector2::new(*dx, *dy);
            }

            _ => {}
        }
    }

    /// Gets a copy of the key's state.
    pub fn key(&self, keycode: VirtualKeyCode) -> BoolAction {
        self.inner.borrow()
            .keys.get(&keycode)
            .map(Clone::clone)
            .unwrap_or_else(Default::default)
    }

    /// Gets a copy of the mouse button's state.
    pub fn button(&self, button: MouseButton) -> BoolAction {
        self.inner.borrow()
            .mouse_buttons.get(&button)
            .map(Clone::clone)
            .unwrap_or_else(Default::default)
    }

    /// Gets the position of the mouse in physical (i.e. display pixel) space. Returns `None` if the
    /// mouse isn't hovering over the window.
    pub fn mouse_pos(&self) -> Option<WindowPosPx> {
        self.inner.borrow()
            .mouse_pos
    }

    /// Gets the sum of all mouse motions in physical (i.e. display pixel) space since the end of
    /// the previous tick.
    pub fn mouse_delta(&self) -> Vector2<f64> {
        self.inner.borrow()
            .mouse_delta
    }

    /// Returns whether or not the viewport has user focus
    pub fn has_focus(&self) -> bool {
        self.inner.borrow().has_focus
    }

    /// Ends the most recent tick period, resetting all "recent" action information.
    pub fn end_tick(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.keys.retain(|_, action| action.end_tick());
        inner.mouse_buttons.retain(|_, action| action.end_tick());
        inner.mouse_delta = Vector2::zero();
    }
}

#[derive(Clone)]
pub struct BoolAction {
    changes: u8,
    state: bool,
}

impl Default for BoolAction {
    fn default() -> Self {
        Self {
            changes: 0,
            state: false,
        }
    }
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

    /// The number of times the action's state changed since the last tick ended.
    pub fn times_changed(&self) -> u8 {
        self.changes
    }

    /// The number of times the action transitioned to a given state since the last tick ended.
    pub fn times_trans(&self, state: bool) -> u8 {
        if self.state == state {
            (self.changes + 1) / 2
        } else {
            self.changes / 2
        }
    }

    /// The number of times the action was pressed since the last tick ended.
    pub fn times_pressed(&self) -> u8 {
        self.times_trans(true)
    }

    /// The number of times the action was released since the last tick ended.
    pub fn times_released(&self) -> u8 {
        self.times_trans(false)
    }

    /// Whether or not the button transitioned to a given state since the last tick ended.
    pub fn recently_became(&self, state: bool) -> bool {
        if self.changes == 1 {
            self.state == state
        } else {
            self.changes != 0
        }
    }

    /// Whether or not the button became pressed since the last tick ended.
    pub fn recently_pressed(&self) -> bool {
        self.recently_became(true)
    }

    /// Whether or not the button became released since the last tick ended.
    pub fn recently_released(&self) -> bool {
        self.recently_became(false)
    }

    /// Signifies the end of the tick. Returns true if the resulting action is identical to `Default`.
    pub fn end_tick(&mut self) -> bool {
        self.changes = 0;
        self.state
    }
}
