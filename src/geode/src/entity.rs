use crate::key::{typed_key, RawTypedKey, TypedKey};
use crate::obj::{Obj, ObjPointee, Session};
use crate::util::arity_utils::impl_tuples;
use antidote::Mutex;
use std::any::Any;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::marker::Unsize;

type EntityMap = HashMap<RawTypedKey, Box<dyn Send + Any>>;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity {
	obj: Obj<EntityInner>,
}

struct EntityInner {
	components: Mutex<EntityMap>,
}

impl Entity {
	pub fn new(session: &Session) -> Self {
		Self {
			obj: Obj::new(
				session,
				EntityInner {
					components: Mutex::new(Default::default()),
				},
			),
		}
	}

	pub fn destroy(&self, session: &Session) {
		self.obj.destroy(session)
	}

	fn inner_add_in<T, K>(&self, session: &Session, _auto_drop: bool, obj: Obj<T>, keys: K)
	where
		T: ?Sized + ObjPointee,
		K: AliasList<T>,
	{
		let mut map = self.obj.get(session).components.lock();
		keys.register_aliases(&mut *map, obj);
	}

	pub fn register_in<T, K>(&self, session: &Session, obj: Obj<T>, keys: K)
	where
		T: ?Sized + ObjPointee,
		K: AliasList<T>,
	{
		self.inner_add_in(session, false, obj, keys);
	}

	pub fn attach_in<T, K>(&self, session: &Session, obj: Obj<T>, keys: K)
	where
		T: ?Sized + ObjPointee,
		K: AliasList<T>,
	{
		self.inner_add_in(session, true, obj, keys);
	}

	pub fn register<T: ?Sized + ObjPointee>(&self, session: &Session, obj: Obj<T>) {
		self.register_in(session, obj, typed_key::<T>());
	}

	pub fn attach<T: ?Sized + ObjPointee>(&self, session: &Session, obj: Obj<T>) {
		self.attach_in(session, obj, typed_key::<T>());
	}

	pub fn get_in<'a, T>(&self, session: &'a Session, key: TypedKey<T>) -> &'a T
	where
		T: ?Sized + ObjPointee,
	{
		self.obj
			.get(session)
			.components
			.lock()
			.get(&key.raw())
			.unwrap()
			.downcast_ref::<Obj<T>>()
			.unwrap()
			.get(session)
	}

	pub fn get<'a, T>(&self, session: &'a Session) -> &'a T
	where
		T: ?Sized + ObjPointee,
	{
		self.get_in(session, typed_key::<T>())
	}

	pub fn borrow<'a, T>(&self, session: &'a Session) -> Ref<'a, T>
	where
		T: ?Sized + ObjPointee,
	{
		self.get::<RefCell<T>>(session).borrow()
	}

	pub fn borrow_mut<'a, T>(&self, session: &'a Session) -> RefMut<'a, T>
	where
		T: ?Sized + ObjPointee,
	{
		self.get::<RefCell<T>>(session).borrow_mut()
	}
}

// === Alias lists === //

pub trait AliasList<T: ?Sized + ObjPointee> {
	#[doc(hidden)]
	fn register_aliases(self, map: &mut EntityMap, obj: Obj<T>);
}

impl<T> AliasList<T> for TypedKey<T>
where
	T: ?Sized + ObjPointee,
{
	fn register_aliases(self, map: &mut EntityMap, obj: Obj<T>) {
		map.insert(self.raw(), Box::new(obj));
	}
}

pub struct UnsizeAs<T>(pub T);

impl<T, U> AliasList<T> for UnsizeAs<TypedKey<U>>
where
	T: ?Sized + ObjPointee + Unsize<U>,
	U: ?Sized + ObjPointee,
{
	fn register_aliases(self, map: &mut EntityMap, obj: Obj<T>) {
		map.insert(self.0.raw(), Box::new(obj.unsize::<U>()));
	}
}

macro tup_impl_alias_list($($name:ident: $field:tt),*) {
	impl<_Src: ?Sized + ObjPointee $(,$name: AliasList<_Src>)*> AliasList<_Src> for ($($name,)*) {
		#[allow(unused_variables)]
		fn register_aliases(self, map: &mut EntityMap, obj: Obj<_Src>) {
			$( self.$field.register_aliases(map, obj); )*
		}
	}
}

impl_tuples!(tup_impl_alias_list);
