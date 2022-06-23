use std::{
	any::Any,
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
};

use derive_where::derive_where;
use parking_lot::Mutex;

use crate::{
	core::{obj::ObjPointee, session::Session},
	util::{arity::impl_tuples, error::UnwrapExt},
	Obj,
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
	pub fn new(session: &Session) -> Self {
		Self {
			obj: Obj::new(session, Default::default()),
		}
	}

	pub fn add<L: ComponentList>(&self, session: &Session, components: L) {
		let map = &mut self.obj.get(session).lock().map;
		components.push_values(&mut ComponentAttachTarget { map });
	}

	pub fn get_in<'a, T: ?Sized + ObjPointee>(
		&self,
		session: &'a Session,
		key: TypedKey<T>,
	) -> &'a T {
		self.obj
			.get(session)
			.lock()
			.map
			.get(&key.raw())
			.unwrap_using(|_| format!("Failed to get component {key:?}"))
			.downcast_ref::<Obj<T>>()
			.unwrap()
			.get(session)
	}

	pub fn get<'a, T: ?Sized + ObjPointee>(&self, session: &'a Session) -> &'a T {
		self.get_in(session, typed_key::<T>())
	}

	pub fn borrow<'a, T: ?Sized + ObjPointee>(&self, session: &'a Session) -> Ref<'a, T> {
		self.get::<RefCell<T>>(session).borrow()
	}

	pub fn borrow_mut<'a, T: ?Sized + ObjPointee>(&self, session: &'a Session) -> RefMut<'a, T> {
		self.get::<RefCell<T>>(session).borrow_mut()
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

impl<T: ?Sized + ObjPointee> ComponentList for Obj<T> {
	fn push_values(self, registry: &mut ComponentAttachTarget) {
		registry.add(typed_key::<T>(), self);
	}
}

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ExposeAs<T: ?Sized + ObjPointee>(pub Obj<T>, pub TypedKey<T>);

impl<T: ?Sized + ObjPointee> ComponentList for ExposeAs<T> {
	fn push_values(self, registry: &mut ComponentAttachTarget) {
		registry.add(self.1, self.0);
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
