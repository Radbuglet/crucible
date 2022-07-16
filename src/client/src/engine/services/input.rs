use std::collections::HashMap;
use std::hash::Hash;
use typed_glam::glam::Vec2;
use winit::dpi::PhysicalPosition;
use winit::event::{
	DeviceEvent, DeviceId, ElementState, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent,
};

/// Tracks keyboard & mouse input states. Users may still need to listen for events to detect
/// certain actions.
pub struct InputTracker {
	keys: HashMap<VirtualKeyCode, BoolAction>,
	mouse_buttons: HashMap<MouseButton, BoolAction>,
	mouse_pos: Option<PhysicalPosition<f64>>,
	mouse_delta: Vec2,
	has_focus: bool,
}

impl Default for InputTracker {
	fn default() -> Self {
		Self {
			keys: Default::default(),
			mouse_buttons: Default::default(),
			mouse_pos: None,
			mouse_delta: Vec2::ZERO,
			has_focus: true,
		}
	}
}

impl InputTracker {
	pub fn handle_window_event(&mut self, event: &WindowEvent) {
		fn set_state_in_map<K: Hash + Eq>(map: &mut HashMap<K, BoolAction>, key: K, value: bool) {
			let action = map.entry(key).or_insert_with(Default::default);
			action.set_state(value);
		}

		match event {
			// On loose focus
			WindowEvent::Focused(has_focus) => {
				if !*has_focus && self.has_focus {
					for key in self.keys.values_mut() {
						key.set_state(false);
					}

					for button in self.mouse_buttons.values_mut() {
						button.set_state(false);
					}

					self.mouse_pos = None;
				}

				self.has_focus = *has_focus;
			}

			// On key update
			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state,
						virtual_keycode: Some(keycode),
						..
					},
				..
			} => {
				set_state_in_map(&mut self.keys, *keycode, *state == ElementState::Pressed);
			}

			// On mouse button update
			WindowEvent::MouseInput { state, button, .. } => {
				set_state_in_map(
					&mut self.mouse_buttons,
					*button,
					*state == ElementState::Pressed,
				);
			}

			// On mouse move
			WindowEvent::CursorMoved { position, .. } => {
				self.mouse_pos = Some(*position);
			}

			// On mouse exit
			WindowEvent::CursorLeft { .. } => {
				self.mouse_pos = None;
			}

			_ => {}
		}
	}

	pub fn handle_device_event(&mut self, _device_id: DeviceId, event: &DeviceEvent) {
		if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
			if self.has_focus {
				self.mouse_delta += Vec2::new(*dx as f32, *dy as f32);
			}
		}
	}

	/// Gets a copy of the key's state.
	pub fn key(&self, keycode: VirtualKeyCode) -> BoolAction {
		self.keys
			.get(&keycode)
			.map(Clone::clone)
			.unwrap_or_else(Default::default)
	}

	/// Gets a copy of the mouse button's state.
	pub fn button(&self, button: MouseButton) -> BoolAction {
		self.mouse_buttons
			.get(&button)
			.map(Clone::clone)
			.unwrap_or_else(Default::default)
	}

	/// Gets the position of the mouse in physical (i.e. display pixel) space. Returns `None` if the
	/// mouse isn't hovering over the window.
	pub fn mouse_pos(&self) -> Option<PhysicalPosition<f64>> {
		self.mouse_pos
	}

	/// Gets the sum of all mouse motions in physical (i.e. display pixel) space since the end of
	/// the previous tick.
	pub fn mouse_delta(&self) -> Vec2 {
		self.mouse_delta
	}

	/// Returns whether or not the viewport has user focus
	pub fn has_focus(&self) -> bool {
		self.has_focus
	}

	/// Ends the most recent tick period, resetting all "recent" action information.
	pub fn end_tick(&mut self) {
		self.keys.retain(|_, action| action.end_tick());
		self.mouse_buttons.retain(|_, action| action.end_tick());
		self.mouse_delta = Vec2::ZERO;
	}
}

#[derive(Debug, Clone, Default)]
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
