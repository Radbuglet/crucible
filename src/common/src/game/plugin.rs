use std::borrow::Cow;

use crucible_util::{c_enum, debug::userdata::BoxedUserdata};
use geode::{Dependent, Entity, EventHandler, Provider, Storage};
use hashbrown::HashMap;

#[derive(Debug, Default)]
pub struct PluginManager {
	plugins: HashMap<Cow<'static, str>, Dependent<Entity>>,
}

impl PluginManager {
	pub fn register(&mut self, (states,): (&mut Storage<PluginState>,), plugin: Entity) {
		self.plugins
			.insert(states[plugin].name.clone(), plugin.into());
	}

	pub fn start(
		&mut self,
		(states, handlers): (&mut Storage<PluginState>, &Storage<PluginLifecycleBase>),
		cx: &Provider,
	) {
		for (id, plugin) in &self.plugins {
			log::info!("Starting plugin {id:?} ({plugin:?})");
			handlers[plugin.get()]
				.on_enable
				.process(&Provider::new_with_parent(cx).with(handlers), plugin.get());
			states[plugin.get()].epoch = PluginEpoch::Running;
		}
	}
}

#[derive(Debug)]
pub struct PluginState {
	name: Cow<'static, str>,
	epoch: PluginEpoch,
}

impl PluginState {
	pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
		Self {
			name: name.into(),
			epoch: PluginEpoch::Idle,
		}
	}
}

c_enum! {
	pub enum PluginEpoch {
		Idle,
		Running,
		Disabled,
	}
}

#[derive(Debug)]
pub struct PluginUserdata(BoxedUserdata);

#[derive(Debug)]
pub struct PluginLifecycleBase {
	pub on_enable: EventHandler<Entity>,
	pub on_disable: EventHandler<Entity>,
}
