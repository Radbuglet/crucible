use crate::util::number::{AtomicU64Generator, NumberGenRef};
use crate::util::reflect::NamedTypeId;
use derive_where::derive_where;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

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
	Static(NamedTypeId),
	Proxy(NamedTypeId),
	Runtime(u64),
}

pub fn typed_key<T: ?Sized + 'static>() -> TypedKey<T> {
	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Static(NamedTypeId::of::<T>())),
	}
}

pub fn proxy_key<T: ?Sized + 'static + ProxyKeyType>() -> TypedKey<T::Provides> {
	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Proxy(NamedTypeId::of::<T>())),
	}
}

pub fn dyn_key<T: ?Sized + 'static>() -> TypedKey<T> {
	static GEN: AtomicU64Generator = AtomicU64Generator::new(1);

	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Runtime(GEN.generate_ref())),
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