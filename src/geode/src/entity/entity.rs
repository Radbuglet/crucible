use std::{
	any::Any,
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
};

use parking_lot::Mutex;

use crate::{
	core::{
		obj::ObjPointee,
		owned::{Destructible, Owned},
		session::Session,
	},
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
	pub fn new(session: &Session) -> Owned<Self> {
		Owned::new(Self {
			obj: Obj::new(session, Default::default()).manually_manage(),
		})
	}

	pub fn new_with<L: ComponentList>(session: &Session, components: L) -> Owned<Self> {
		let mut inner = EntityInner::default();

		components.push_values(&mut ComponentAttachTarget {
			map: &mut inner.map,
		});

		Owned::new(Self {
			obj: Obj::new(session, Mutex::new(inner)).manually_manage(),
		})
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
			.unwrap_using(|_| format!("Missing component under key {key:?}"))
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

	pub fn destroy(&self, session: &Session) {
		self.obj.destroy(session)
	}
}

impl Destructible for Entity {
	fn destruct(self) {
		self.destroy(&Session::new([]))
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
		registry.add(key, self.manually_manage());
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
