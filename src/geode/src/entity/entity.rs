use std::{
	any::Any,
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
};

use crucible_core::{error::ResultExt, macros::impl_tuples};
use derive_where::derive_where;
use parking_lot::Mutex;
use thiserror::Error;

use crate::core::{
	obj::{Obj, ObjGetError, ObjLockedError, ObjPointee},
	owned::{Destructible, Owned},
	session::{LocalSessionGuard, Session},
};

use super::key::{typed_key, RawTypedKey, TypedKey};

// === `Entity` core === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
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

	pub fn fallible_get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> Result<&'a T, EntityGetError> {
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
			.unwrap()
			// Deref component
			.try_get(session)
			.map_err(|err| EntityGetError::CompDerefError(key.raw(), err))?)
	}

	pub fn fallible_get<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
	) -> Result<&'a T, EntityGetError> {
		self.fallible_get_in(session, typed_key::<T>())
	}

	pub fn get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> &'a T {
		self.fallible_get_in(session, key).unwrap_pretty()
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

	pub fn destroy(&self, session: Session) -> bool {
		self.obj.destroy(session)
	}
}

impl Owned<Entity> {
	pub fn add<L: ComponentList>(&self, session: Session, components: L) {
		self.weak_copy().add(session, components)
	}

	pub fn fallible_get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> Result<&'a T, EntityGetError> {
		self.weak_copy().fallible_get_in(session, key)
	}

	pub fn fallible_get<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
	) -> Result<&'a T, EntityGetError> {
		self.weak_copy().fallible_get(session)
	}

	pub fn get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> &'a T {
		self.weak_copy().get_in(session, key)
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

	pub fn destroy(self, session: Session) -> bool {
		self.manually_destruct().destroy(session)
	}
}

impl Destructible for Entity {
	fn destruct(self) {
		LocalSessionGuard::with_new(|session| {
			self.destroy(session.handle());
		});
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

pub trait EntityGetErrorExt {
	type OkTy;

	fn ok_or_missing(self) -> Result<Self::OkTy, ComponentMissingError>;
}

impl<T> EntityGetErrorExt for Result<T, EntityGetError> {
	type OkTy = T;

	fn ok_or_missing(self) -> Result<Self::OkTy, ComponentMissingError> {
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

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ExposeUsing<T: SingleComponent>(pub T, pub TypedKey<T::Value>);

impl<T: SingleComponent> ComponentList for ExposeUsing<T> {
	fn push_values(self, registry: &mut ComponentAttachTarget) {
		self.0.push_value_under(registry, self.1);
	}
}

macro impl_component_list_on_tuple($($name:ident: $field:tt),*) {
    impl<$($name: ComponentList),*> ComponentList for ($($name,)*) {
        #[allow(unused)]
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

#[derive_where(Debug)]
pub enum OwnedOrWeak<T: ?Sized + ObjPointee> {
	Owned(Owned<Obj<T>>),
	Weak(Obj<T>),
}

impl<T: ?Sized + ObjPointee> SingleComponent for OwnedOrWeak<T> {
	type Value = T;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>) {
		match self {
			OwnedOrWeak::Owned(owned) => registry.add_owned(key, owned),
			OwnedOrWeak::Weak(weak) => registry.add_weak(key, weak),
		}
	}
}

impl<T: ?Sized + ObjPointee> From<Owned<Obj<T>>> for OwnedOrWeak<T> {
	fn from(owned: Owned<Obj<T>>) -> Self {
		Self::Owned(owned)
	}
}

impl<T: ?Sized + ObjPointee> From<Obj<T>> for OwnedOrWeak<T> {
	fn from(weak: Obj<T>) -> Self {
		Self::Weak(weak)
	}
}

// `Option<OwnedOrWeak<T>>` conversions

impl<T: ?Sized + ObjPointee> SingleComponent for Option<OwnedOrWeak<T>> {
	type Value = T;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>) {
		match self {
			Some(OwnedOrWeak::Owned(owned)) => registry.add_owned(key, owned),
			Some(OwnedOrWeak::Weak(weak)) => registry.add_weak(key, weak),
			None => {}
		}
	}
}

impl<T: ?Sized + ObjPointee> From<Owned<Obj<T>>> for Option<OwnedOrWeak<T>> {
	fn from(owned: Owned<Obj<T>>) -> Self {
		Some(OwnedOrWeak::Owned(owned))
	}
}

impl<T: ?Sized + ObjPointee> From<Obj<T>> for Option<OwnedOrWeak<T>> {
	fn from(weak: Obj<T>) -> Self {
		Some(OwnedOrWeak::Weak(weak))
	}
}
