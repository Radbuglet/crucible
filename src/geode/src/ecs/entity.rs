use std::{
	any::Any,
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
	fmt,
};

use crucible_core::{error::ResultExt, macros::impl_tuples, std_traits::ResultLike};
use parking_lot::Mutex;
use thiserror::Error;

use crate::core::{
	obj::{Obj, ObjGetError, ObjLockedError, ObjPointee},
	owned::{Destructible, MaybeOwned, Owned},
	session::{LocalSessionGuard, Session},
};

use super::key::{typed_key, RawTypedKey, TypedKey};

// === `Entity` core === //

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity {
	obj: Obj<Mutex<EntityInner>>,
}

#[derive(Default)]
struct EntityInner {
	map: HashMap<RawTypedKey, Box<dyn Any + Send>>,
}

impl Entity {
	pub fn new(session: Session) -> Owned<Self> {
		Owned::new(Self {
			obj: Obj::new(session, Default::default()).manually_destruct(),
		})
	}

	pub fn new_with<L: ComponentList>(session: Session, components: L) -> Owned<Self> {
		let mut inner = EntityInner::default();

		components.push_values(&mut ComponentAttachTarget {
			map: &mut inner.map,
		});

		Owned::new(Self {
			obj: Obj::new(session, Mutex::new(inner)).manually_destruct(),
		})
	}

	pub fn add<L: ComponentList>(&self, session: Session, components: L) {
		let map = &mut self.obj.get(session).lock().map;
		components.push_values(&mut ComponentAttachTarget { map });
	}

	pub fn fallible_get_obj_in<T: ?Sized + ObjPointee>(
		&self,
		session: Session,
		key: TypedKey<T>,
	) -> Result<Obj<T>, EntityGetError> {
		Ok(self
			.obj
			// Acquire `Entity` heap handle
			.try_get(session)
			.map_err(EntityGetError::EntityDerefError)?
			// Lock hash map
			.lock()
			.map
			// Get component in `HashMap`
			.get(&key.raw())
			.ok_or_else(|| {
				EntityGetError::ComponentMissing(ComponentMissingError { key: key.raw() })
			})?
			.downcast_ref::<Obj<T>>()
			.copied()
			.unwrap())
	}

	pub fn fallible_get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> Result<&'a T, EntityGetError> {
		self.fallible_get_obj_in(session, key)?
			.try_get(session)
			.map_err(|err| EntityGetError::CompDerefError(key.raw(), err))
	}

	pub fn fallible_get_obj<T: ?Sized + ObjPointee>(
		&self,
		session: Session,
	) -> Result<Obj<T>, EntityGetError> {
		self.fallible_get_obj_in(session, typed_key::<T>())
	}

	pub fn fallible_get<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
	) -> Result<&'a T, EntityGetError> {
		self.fallible_get_in(session, typed_key::<T>())
	}

	pub fn get_obj_in<T: ?Sized + ObjPointee>(&self, session: Session, key: TypedKey<T>) -> Obj<T> {
		self.fallible_get_obj_in(session, key).unwrap_pretty()
	}

	pub fn get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> &'a T {
		self.fallible_get_in(session, key).unwrap_pretty()
	}

	pub fn get_obj<T: ?Sized + ObjPointee>(&self, session: Session) -> Obj<T> {
		self.get_obj_in(session, typed_key::<T>())
	}

	pub fn get<'a, T: ?Sized + ObjPointee>(&self, session: Session<'a>) -> &'a T {
		self.get_in(session, typed_key::<T>())
	}

	pub fn borrow<'a, T: ?Sized + ObjPointee>(&self, session: Session<'a>) -> Ref<'a, T> {
		self.get::<RefCell<T>>(session).borrow()
	}

	pub fn borrow_mut<'a, T: ?Sized + ObjPointee>(&self, session: Session<'a>) -> RefMut<'a, T> {
		self.get::<RefCell<T>>(session).borrow_mut()
	}

	pub fn is_alive_now(&self, session: Session) -> bool {
		self.obj.is_alive_now(session)
	}

	pub fn destroy(&self, session: Session) -> bool {
		self.obj.destroy(session)
	}
}

impl Destructible for Entity {
	fn destruct(self) {
		LocalSessionGuard::with_new(|session| {
			self.destroy(session.handle());
		});
	}
}

impl fmt::Debug for Entity {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let session = LocalSessionGuard::new();
		let s = session.handle();

		let keys = self
			.obj
			.try_get(s)
			.map(|mutex| mutex.lock().map.keys().copied().collect::<Vec<_>>());

		f.debug_struct("Entity")
			.field("gen", &self.obj.ptr_gen())
			.field("components", &keys)
			.finish()
	}
}

// === `Owned<Entity>` Forwards === //

impl Owned<Entity> {
	pub fn add<L: ComponentList>(&self, session: Session, components: L) {
		self.weak_copy().add(session, components)
	}

	pub fn fallible_get_obj_in<T: ?Sized + ObjPointee>(
		&self,
		session: Session,
		key: TypedKey<T>,
	) -> Result<Obj<T>, EntityGetError> {
		self.weak_copy().fallible_get_obj_in(session, key)
	}

	pub fn fallible_get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> Result<&'a T, EntityGetError> {
		self.weak_copy().fallible_get_in(session, key)
	}

	pub fn fallible_get_obj<T: ?Sized + ObjPointee>(
		&self,
		session: Session,
	) -> Result<Obj<T>, EntityGetError> {
		self.weak_copy().fallible_get_obj(session)
	}

	pub fn fallible_get<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
	) -> Result<&'a T, EntityGetError> {
		self.weak_copy().fallible_get(session)
	}

	pub fn get_obj_in<T: ?Sized + ObjPointee>(&self, session: Session, key: TypedKey<T>) -> Obj<T> {
		self.weak_copy().get_obj_in(session, key)
	}

	pub fn get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> &'a T {
		self.weak_copy().get_in(session, key)
	}

	pub fn get_obj<T: ?Sized + ObjPointee>(&self, session: Session) -> Obj<T> {
		self.weak_copy().get_obj(session)
	}

	pub fn get<'a, T: ?Sized + ObjPointee>(&self, session: Session<'a>) -> &'a T {
		self.weak_copy().get(session)
	}

	pub fn borrow<'a, T: ?Sized + ObjPointee>(&self, session: Session<'a>) -> Ref<'a, T> {
		self.weak_copy().borrow(session)
	}

	pub fn borrow_mut<'a, T: ?Sized + ObjPointee>(&self, session: Session<'a>) -> RefMut<'a, T> {
		self.weak_copy().borrow_mut(session)
	}

	pub fn is_alive_now(&self, session: Session) -> bool {
		self.weak_copy().is_alive_now(session)
	}

	pub fn destroy(self, session: Session) -> bool {
		self.manually_destruct().destroy(session)
	}
}

impl MaybeOwned<Entity> {
	pub fn add<L: ComponentList>(&self, session: Session, components: L) {
		self.weak_copy().add(session, components)
	}

	pub fn fallible_get_obj_in<T: ?Sized + ObjPointee>(
		&self,
		session: Session,
		key: TypedKey<T>,
	) -> Result<Obj<T>, EntityGetError> {
		self.weak_copy().fallible_get_obj_in(session, key)
	}

	pub fn fallible_get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> Result<&'a T, EntityGetError> {
		self.weak_copy().fallible_get_in(session, key)
	}

	pub fn fallible_get_obj<T: ?Sized + ObjPointee>(
		&self,
		session: Session,
	) -> Result<Obj<T>, EntityGetError> {
		self.weak_copy().fallible_get_obj(session)
	}

	pub fn fallible_get<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
	) -> Result<&'a T, EntityGetError> {
		self.weak_copy().fallible_get(session)
	}

	pub fn get_obj_in<T: ?Sized + ObjPointee>(&self, session: Session, key: TypedKey<T>) -> Obj<T> {
		self.weak_copy().get_obj_in(session, key)
	}

	pub fn get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> &'a T {
		self.weak_copy().get_in(session, key)
	}

	pub fn get_obj<T: ?Sized + ObjPointee>(&self, session: Session) -> Obj<T> {
		self.weak_copy().get_obj(session)
	}

	pub fn get<'a, T: ?Sized + ObjPointee>(&self, session: Session<'a>) -> &'a T {
		self.weak_copy().get(session)
	}

	pub fn borrow<'a, T: ?Sized + ObjPointee>(&self, session: Session<'a>) -> Ref<'a, T> {
		self.weak_copy().borrow(session)
	}

	pub fn borrow_mut<'a, T: ?Sized + ObjPointee>(&self, session: Session<'a>) -> RefMut<'a, T> {
		self.weak_copy().borrow_mut(session)
	}

	pub fn is_alive_now(&self, session: Session) -> bool {
		self.weak_copy().is_alive_now(session)
	}

	pub fn destroy(self, session: Session) -> bool {
		self.manually_destruct().destroy(session)
	}
}

// === `Entity` error types === //

#[derive(Debug, Copy, Clone, Error)]
pub enum EntityGetError {
	#[error("failed to deref `Entity`'s `Obj`")]
	EntityDerefError(#[source] ObjGetError),
	#[error("failed to deref `Obj` of component with key {0:?}")]
	CompDerefError(RawTypedKey, #[source] ObjGetError),
	#[error("failed to get missing component")]
	ComponentMissing(#[source] ComponentMissingError),
}

impl EntityGetError {
	pub fn as_missing_error(self) -> Result<ComponentMissingError, EntityGetError> {
		match self {
			Self::ComponentMissing(err) => Ok(err),
			other => Err(other),
		}
	}

	pub fn as_permission_error(self) -> Option<ObjLockedError> {
		match self {
			Self::EntityDerefError(ObjGetError::Locked(perm_err)) => Some(perm_err),
			Self::CompDerefError(_, ObjGetError::Locked(perm_err)) => Some(perm_err),
			_ => None,
		}
	}

	pub fn ok_or_missing<T>(result: Result<T, Self>) -> Result<T, ComponentMissingError> {
		result.map_err(|err| err.as_missing_error().unwrap_pretty())
	}
}

pub trait EntityGetErrorExt: ResultLike {
	fn ok_or_missing(self) -> Result<Self::Success, ComponentMissingError>;
}

impl<T> EntityGetErrorExt for Result<T, EntityGetError> {
	fn ok_or_missing(self) -> Result<Self::Success, ComponentMissingError> {
		EntityGetError::ok_or_missing(self)
	}
}

#[derive(Debug, Copy, Clone, Error)]
#[error("component with key {key:?} is missing from `Entity`")]
pub struct ComponentMissingError {
	pub key: RawTypedKey,
}

// === `ComponentList` === //

pub struct ComponentAttachTarget<'a> {
	map: &'a mut HashMap<RawTypedKey, Box<dyn Any + Send>>,
}

impl ComponentAttachTarget<'_> {
	pub fn add_raw<T: ?Sized + ObjPointee>(
		&mut self,
		key: TypedKey<T>,
		value: Obj<T>,
		_call_dctor: bool,
	) {
		self.map.insert(key.raw(), Box::new(value));
	}

	pub fn add_weak<T: ?Sized + ObjPointee>(&mut self, key: TypedKey<T>, value: Obj<T>) {
		self.add_raw(key, value, false);
	}

	pub fn add_owned<T: ?Sized + ObjPointee>(&mut self, key: TypedKey<T>, value: Owned<Obj<T>>) {
		self.add_raw(key, value.manually_destruct(), true);
	}
}

pub trait ComponentList: Sized {
	fn push_values(self, registry: &mut ComponentAttachTarget);
}

impl<T: SingleComponent> ComponentList for T {
	fn push_values(self, registry: &mut ComponentAttachTarget) {
		self.push_value_under(registry, typed_key::<T::Value>());
	}
}

#[allow(clippy::derive_partial_eq_without_eq)] // False positive??
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct ExposeUsing<T: SingleComponent>(pub T, pub TypedKey<T::Value>);

impl<T: SingleComponent> ComponentList for ExposeUsing<T> {
	fn push_values(self, registry: &mut ComponentAttachTarget) {
		self.0.push_value_under(registry, self.1);
	}
}

macro impl_component_list_on_tuple($($name:ident: $field:tt),*) {
    impl<$($name: ComponentList),*> ComponentList for ($($name,)*) {
        #[allow(unused)]  // `registry` unused in unit tuples
        fn push_values(self, registry: &mut ComponentAttachTarget) {
            $(self.$field.push_values(registry);)*
        }
    }
}

impl_tuples!(impl_component_list_on_tuple);

// === `SingleComponent` === //

pub trait SingleComponent: Sized {
	type Value: ?Sized + ObjPointee;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>);
}

impl<T: ?Sized + ObjPointee> SingleComponent for Obj<T> {
	type Value = T;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>) {
		registry.add_weak(key, self);
	}
}

impl<T: ?Sized + ObjPointee> SingleComponent for Owned<Obj<T>> {
	type Value = T;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>) {
		registry.add_owned(key, self);
	}
}

impl<T: ?Sized + ObjPointee> SingleComponent for MaybeOwned<Obj<T>> {
	type Value = T;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>) {
		match self {
			MaybeOwned::Owned(owned) => registry.add_owned(key, owned),
			MaybeOwned::Weak(weak) => registry.add_weak(key, weak),
		}
	}
}

impl<T: SingleComponent> SingleComponent for Option<T> {
	type Value = T::Value;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>) {
		if let Some(val) = self {
			val.push_value_under(registry, key);
		}
	}
}