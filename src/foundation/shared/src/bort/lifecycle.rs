use bort::{access_cx, CompMut, CompRef, Entity};
use crucible_util::{
	debug::type_id::NamedTypeId,
	mem::hash::{FxHashMap, FxHashSet},
};
use derive_where::derive_where;

// === PartialEntity === //

#[derive(Debug, Copy, Clone)]
pub struct PartialEntity<'a> {
	target: Entity,
	can_access: &'a FxHashSet<NamedTypeId>,
}

impl PartialEntity<'_> {
	pub fn add<T: 'static>(self, component: T) {
		assert!(!self.target.has::<T>());
		self.target.insert(component);
	}

	pub fn get_s<'a, T: 'static>(self, cx: &'a access_cx![ref T]) -> CompRef<'a, T> {
		assert!(self.can_access.contains(&NamedTypeId::of::<T>()));
		self.target.get_s(cx)
	}

	pub fn get_mut_s<'a, T: 'static>(self, cx: &'a access_cx![mut T]) -> CompMut<'a, T> {
		assert!(self.can_access.contains(&NamedTypeId::of::<T>()));
		self.target.get_mut_s(cx)
	}

	pub fn entity(self) -> Entity {
		self.target
	}
}

// === LifecycleManager === //

#[derive(Debug)]
#[derive_where(Default)]
pub struct LifecycleManager<I> {
	handlers: Vec<Handler<I>>,
	handlers_with_deps: FxHashMap<NamedTypeId, Vec<usize>>,
	handlers_without_any_deps: Vec<usize>,
}

#[derive(Debug)]
struct Handler<I> {
	delegate: I,
	deps: FxHashSet<NamedTypeId>,
}

impl<I> LifecycleManager<I> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with(mut self, deps: impl IntoIterator<Item = NamedTypeId>, delegate: I) -> Self {
		self.register(deps, delegate);
		self
	}

	pub fn with_many(mut self, f: impl FnOnce(&mut LifecycleManager<I>)) -> Self {
		self.register_many(f);
		self
	}

	pub fn register(
		&mut self,
		deps: impl IntoIterator<Item = NamedTypeId>,
		delegate: I,
	) -> &mut Self {
		let deps = deps.into_iter().collect::<FxHashSet<_>>();
		if deps.is_empty() {
			self.handlers_without_any_deps.push(self.handlers.len());
		} else {
			for &dep in &deps {
				self.handlers_with_deps
					.entry(dep)
					.or_default()
					.push(self.handlers.len());
			}
		}
		self.handlers.push(Handler { delegate, deps });

		self
	}

	pub fn register_many(&mut self, f: impl FnOnce(&mut LifecycleManager<I>)) -> &mut Self {
		f(self);
		self
	}

	pub fn execute(&self, mut executor: impl FnMut(&I, PartialEntity<'_>), target: Entity) {
		// Execute handlers without dependencies
		for &handler in &self.handlers_without_any_deps {
			executor(
				&self.handlers[handler].delegate,
				PartialEntity {
					target,
					can_access: &self.handlers[handler].deps,
				},
			)
		}

		// Execute handlers with dependencies
		let mut remaining_dep_types = self.handlers_with_deps.keys().copied().collect::<Vec<_>>();
		let mut dep_counts = self
			.handlers
			.iter()
			.map(|handler| handler.deps.len())
			.collect::<Vec<_>>();

		while !remaining_dep_types.is_empty() {
			let old_len = remaining_dep_types.len();

			remaining_dep_types.retain(|&dep| {
				if !target.has_dyn(dep.into()) {
					return true;
				}

				for &handler in &self.handlers_with_deps[&dep] {
					dep_counts[handler] -= 1;

					if dep_counts[handler] == 0 {
						executor(
							&self.handlers[handler].delegate,
							PartialEntity {
								target,
								can_access: &self.handlers[handler].deps,
							},
						);
					}
				}

				false
			});

			assert_ne!(
				remaining_dep_types.len(),
				old_len,
				"PluginLoader is unable to load the following required component types: {:?}",
				remaining_dep_types
			);
		}
	}
}
