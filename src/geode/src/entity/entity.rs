use std::{
	any::Any,
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
};

use crucible_core::{arity::impl_tuples, error::ResultExt};
use parking_lot::Mutex;
use thiserror::Error;

use crate::core::{
	obj::{Obj, ObjGetError, ObjPointee},
	owned::{Destructible, Owned},
	session::{LocalSessionGuard, Session},
};

use super::key::{typed_key, RawTypedKey, TypedKey};

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

	pub fn falliable_get_in<'a, T: ?Sized + ObjPointee>(
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

	pub fn falliable_get<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
	) -> Result<&'a T, EntityGetError> {
		self.falliable_get_in(session, typed_key::<T>())
	}

	pub fn get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: Session<'a>,
		key: TypedKey<T>,
	) -> &'a T {
		self.falliable_get_in(session, key).unwrap_pretty()
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

impl Destructible for Entity {
	fn destruct(self) {
		LocalSessionGuard::with_new(|session| {
			self.destroy(session.handle());
		});
	}
}

pub trait RegisterObj: Sized {
	type Value: ?Sized + ObjPointee;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>);
}

impl<T: ?Sized + ObjPointee> RegisterObj for Obj<T> {
	type Value = T;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>) {
		registry.add(key, self);
	}
}

impl<T: ?Sized + ObjPointee> RegisterObj for Owned<Obj<T>> {
	type Value = T;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>) {
		registry.add(key, self.manually_destruct());
	}
}

pub struct ComponentAttachTarget<'a> {
	map: &'a mut HashMap<RawTypedKey, Box<dyn Any + Send>>,
}

impl ComponentAttachTarget<'_> {
	pub fn add<T: ?Sized + ObjPointee>(&mut self, key: TypedKey<T>, value: Obj<T>) {
		self.map.insert(key.raw(), Box::new(value));
	}
}

pub trait ComponentList: Sized {
	fn push_values(self, registry: &mut ComponentAttachTarget);
}

impl<T: RegisterObj> ComponentList for T {
	fn push_values(self, registry: &mut ComponentAttachTarget) {
		self.push_value_under(registry, typed_key::<T::Value>());
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ExposeUsing<T: RegisterObj>(pub T, pub TypedKey<T::Value>);

impl<T: RegisterObj> ComponentList for ExposeUsing<T> {
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

#[derive(Debug, Clone, Error)]
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

#[derive(Debug, Clone, Error)]
#[error("component with key {key:?} is missing from `Entity`")]
pub struct ComponentMissingError {
	pub key: RawTypedKey,
}
