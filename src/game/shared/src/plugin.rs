use std::borrow::Cow;

use bort::{Entity, OwnedEntity};
use crucible_util::delegate;
use semver::Version;
use smallvec::SmallVec;

// === BasePlugin === //

delegate! {
	pub fn PluginLifecycleHandler(plugin: &mut BasePlugin)
}

#[derive(Debug)]
pub struct BasePlugin {
	me: Entity,
	config: PluginConfig,
}

impl BasePlugin {
	pub fn new(me: Entity, config: PluginConfig) -> Self {
		Self { me, config }
	}

	pub fn entity(&self) -> Entity {
		self.me
	}

	pub fn config(&self) -> &PluginConfig {
		&self.config
	}
}

#[derive(Debug, Clone)]
pub struct PluginConfig {
	pub id: Cow<'static, str>,
	pub api_version: Version,
	pub dependencies: Vec<Cow<'static, str>>,
	pub meta: PluginConfigMeta,
	pub on_enable: PluginLifecycleHandler,
	pub on_disable: PluginLifecycleHandler,
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct PluginConfigMeta {
	pub namespace: Option<Cow<'static, str>>,
	pub name: Option<Cow<'static, str>>,
	pub description: Option<Cow<'static, str>>,
	pub authors: Option<SmallVec<[Cow<'static, str>; 1]>>,
	pub website: Option<Cow<'static, str>>,
}

impl PluginConfigMeta {
	pub fn with_namespace(mut self, namespace: impl Into<Cow<'static, str>>) -> Self {
		self.namespace = Some(namespace.into());
		self
	}

	pub fn with_name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
		self.name = Some(name.into());
		self
	}

	pub fn with_description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
		self.description = Some(description.into());
		self
	}

	pub fn with_authors(
		mut self,
		authors: impl IntoIterator<Item = impl Into<Cow<'static, str>>>,
	) -> Self {
		self.authors = Some(SmallVec::from_iter(authors.into_iter().map(Into::into)));
		self
	}

	pub fn with_website(mut self, website: impl Into<Cow<'static, str>>) -> Self {
		self.website = Some(website.into());
		self
	}
}

// === BoilerPlugin === //

pub trait BoilerPlugin: 'static + Default {
	// === Required === //

	const ID: &'static str;
	const API_VERSION: Version;

	fn meta() -> PluginConfigMeta;

	fn on_enable(&mut self, plugin: &mut BasePlugin);

	fn on_disable(&mut self, plugin: &mut BasePlugin);

	// === Provided === //

	fn spawn_entity() -> OwnedEntity {
		OwnedEntity::new()
			.with_debug_label(format_args!("plugin root for {:?}", Self::ID))
			.with_self_referential(|me| {
				BasePlugin::new(
					me,
					PluginConfig {
						id: Cow::Borrowed(Self::ID),
						api_version: Self::API_VERSION,
						dependencies: vec![],
						meta: Self::meta(),
						on_enable: PluginLifecycleHandler::new(|plugin| {
							plugin.entity().get_mut::<Self>().on_enable(plugin);
						}),
						on_disable: PluginLifecycleHandler::new(|plugin| {
							plugin.entity().get_mut::<Self>().on_disable(plugin);
						}),
					},
				)
			})
			.with(Self::default())
	}
}
