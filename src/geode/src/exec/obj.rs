use crate::util::arity_utils::impl_tuples;
use crate::util::component::{Component, FancyTypeId};
use crate::util::error::ResultExt;
use crate::util::number::NumberGenRef;
use bumpalo::Bump;
use derive_where::derive_where;
use std::alloc::Layout;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;
use std::marker::{PhantomData, Unsize};
use std::ptr::NonNull;
use std::sync::atomic::AtomicU64;
use thiserror::Error;

// === Obj core === //

#[derive(Debug, Default)]
pub struct Obj {
	comps: HashMap<RawTypedKey, Component>,
	bump: Bump,
}

// `Obj` is `Send` and `Sync` because all components inserted into it must also be `Send` and `Sync`.
unsafe impl Send for Obj {}
unsafe impl Sync for Obj {}

impl Obj {
	pub fn add<T>(&mut self, value: T)
	where
		T: Sized + Send + Sync + 'static,
	{
		self.add_as(typed_key::<T>(), value, ());
	}

	pub fn add_as<T, A>(&mut self, owning_key: TypedKey<T>, value: T, alias_as: A)
	where
		T: Sized + Send + Sync,
		A: AliasList<T>,
	{
		// Ensure that we haven't already registered this key.
		let owning_key = owning_key.raw();
		assert!(!self.comps.contains_key(&owning_key));

		// Allocate component
		let comp = self.bump.alloc_layout(Layout::new::<T>()).cast::<T>();
		unsafe {
			comp.as_ptr().write(value);
		}

		// Register the principal entry
		#[rustfmt::skip]
		self.comps.insert(owning_key, Component::new_owned(comp, &mut self.bump));

		// Register alias entries
		unsafe {
			alias_as.push_aliases(self, comp);
		}
	}

	pub fn try_get_raw<T: ?Sized>(&self, key: TypedKey<T>) -> Option<NonNull<T>> {
		let entry = self.comps.get(&key.raw())?;
		Some(unsafe { entry.target_ptr::<T>() })
	}
}

impl Drop for Obj {
	fn drop(&mut self) {
		for comp in self.comps.values_mut() {
			unsafe {
				comp.drop_if_owned();
			}
		}
	}
}

// === Multi-fetch === //

pub trait ObjBorrowable<'a>: Sized {
	fn try_borrow_from(obj: &'a Obj) -> Result<Self, BorrowError>;
}

// impl<'a, T: ?Sized + 'static> ObjBorrowable for RwMut<'a, T> {
// 	fn try_borrow_from(obj: &Obj) -> Result<Self, BorrowError> {
// 		obj.try_borrow_mut()
// 	}
// }
//
// impl<T: ?Sized + 'static> ObjBorrowable for RwRef<T> {
// 	fn try_borrow_from(obj: &Obj) -> Result<Self, BorrowError> {
// 		obj.try_borrow_ref()
// 	}
// }

macro impl_tup_obj_borrowable($($name:ident: $field:tt),*) {
	impl<'a, $($name: ObjBorrowable<'a>),*> ObjBorrowable<'a> for ($($name,)*) {
		#[allow(unused_variables)]
		fn try_borrow_from(obj: &'a Obj) -> Result<Self, BorrowError> {
			Ok(($($name::try_borrow_from(obj)?,)*))
		}
	}
}

impl_tuples!(impl_tup_obj_borrowable);

// === Errors === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum LockState {
	Mutably,
	Immutably(usize),
	Unborrowed,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
#[error("Component {key:?} missing from `Obj`.")]
pub struct ComponentMissingError {
	key: RawTypedKey,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct LockError {
	state: LockState,
	key: RawTypedKey,
}

impl Error for LockError {}

impl Display for LockError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "Failed to lock component with key {:?}", self.key)?;
		match self.state {
			LockState::Mutably => {
				f.write_str(
					"immutably: 1 concurrent mutable borrow prevents shared immutable access.",
				)?;
			}
			LockState::Immutably(concurrent) => {
				write!(
					f,
					"mutably: {} concurrent immutable borrow{} prevent{} exclusive mutable access.",
					concurrent,
					// Gotta love English grammar
					if concurrent == 1 { "" } else { "s" },
					if concurrent == 1 { "s" } else { "" },
				)?;
			}
			LockState::Unborrowed => {
				#[cfg(debug_assertions)]
				unreachable!();
				#[cfg(not(debug_assertions))]
				f.write_str("even though it was unborrowed?!")?;
			}
		}
		Ok(())
	}
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
pub enum BorrowError {
	#[error("Failed to borrow. {0}")]
	ComponentMissing(ComponentMissingError),
	#[error("Failed to borrow. {0}")]
	LockError(LockError),
}

// === Keys === //

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct TypedKey<T: ?Sized> {
	_ty: PhantomData<fn(T) -> T>,
	raw: RawTypedKey,
}

impl<T: ?Sized> TypedKey<T> {
	pub unsafe fn from_raw(raw: RawTypedKey) -> TypedKey<T> {
		Self {
			_ty: PhantomData,
			raw,
		}
	}

	pub fn raw(&self) -> RawTypedKey {
		self.raw
	}
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct RawTypedKey(TypedKeyRawInner);

impl Debug for RawTypedKey {
	#[rustfmt::skip]
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match &self.0 {
			TypedKeyRawInner::Static(key) => {
				f.debug_tuple("RawTypedKey::Static").field(key).finish()
			}
			TypedKeyRawInner::Proxy(key) => {
				f.debug_tuple("RawTypedKey::Proxy").field(key).finish()
			}
			TypedKeyRawInner::Runtime(key) => {
				f.debug_tuple("RawTypedKey::Runtime").field(key).finish()
			}
		}
	}
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
enum TypedKeyRawInner {
	Static(FancyTypeId),
	Proxy(FancyTypeId),
	Runtime(u64),
}

pub fn typed_key<T: ?Sized + 'static>() -> TypedKey<T> {
	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Static(FancyTypeId::of::<T>())),
	}
}

pub fn proxy_key<T: ?Sized + 'static + ProxyKeyType>() -> TypedKey<T::Provides> {
	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Proxy(FancyTypeId::of::<T>())),
	}
}

pub fn dyn_key<T: ?Sized + 'static>() -> TypedKey<T> {
	static GEN: AtomicU64 = AtomicU64::new(0);

	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Runtime(
			GEN.try_generate_ref().unwrap_pretty(),
		)),
	}
}

#[doc(hidden)]
pub trait ProxyKeyType {
	type Provides: ?Sized + 'static;
}

pub macro proxy_key($(
	$(#[$macro_meta:meta])*
	$vis:vis proxy $name:ident($target:ty);
)*) {$(
	$(#[$macro_meta])*
	$vis struct $name;

	impl ProxyKeyType for $name {
		type Provides = $target;
	}
)*}

// === Alias lists === //

pub unsafe trait AliasList<T: Sized> {
	unsafe fn push_aliases(self, map: &mut Obj, ptr: NonNull<T>);
}

unsafe impl<T, U> AliasList<T> for TypedKey<U>
where
	T: Sized + Unsize<U>,
	U: ?Sized + 'static,
{
	unsafe fn push_aliases(self, map: &mut Obj, ptr: NonNull<T>) {
		// Unsize the value and convert it back into a pointer
		let ptr = (ptr.as_ref() as &U) as *const U as *mut U;
		let ptr = NonNull::new_unchecked(ptr);

		// Insert the entry
		#[rustfmt::skip]
		map.comps.insert(
			self.raw(),
			Component::new_alias(ptr, &mut map.bump)
		);
	}
}

macro tup_impl_alias_list($($name:ident: $field:tt),*) {
unsafe impl<_Src: Sized $(,$name: AliasList<_Src>)*> AliasList<_Src> for ($($name,)*) {
		#[allow(unused_variables)]
		unsafe fn push_aliases(self, obj: &mut Obj, ptr: NonNull<_Src>) {
			$( self.$field.push_aliases(obj, ptr); )*
		}
	}
}

impl_tuples!(tup_impl_alias_list);
