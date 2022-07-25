use std::{
	fmt, hash,
	marker::PhantomData,
	sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
};

use crucible_core::{marker::PhantomInvariant, type_id::NamedTypeId};

// === `TypedKey` === //

pub struct TypedKey<T: ?Sized> {
	_ty: PhantomInvariant<T>,
	raw: RawTypedKey,
}

impl<T: ?Sized> fmt::Debug for TypedKey<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match &self.raw.0 {
			TypedKeyRawInner::Static(key) => f.debug_tuple("TypedKey::Static").field(key).finish(),
			TypedKeyRawInner::Proxy(key) => f.debug_tuple("TypedKey::Proxy").field(key).finish(),
			TypedKeyRawInner::Runtime(key) => {
				f.debug_tuple("TypedKey::Runtime").field(key).finish()
			}
		}
	}
}

impl<T: ?Sized> Copy for TypedKey<T> {}

impl<T: ?Sized> Clone for TypedKey<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized> hash::Hash for TypedKey<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.raw.hash(state);
	}
}

impl<T: ?Sized> Eq for TypedKey<T> {}

impl<T: ?Sized> PartialEq for TypedKey<T> {
	fn eq(&self, other: &Self) -> bool {
		self.raw == other.raw
	}
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

// === `RawTypedKey` === //

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct RawTypedKey(TypedKeyRawInner);

impl fmt::Debug for RawTypedKey {
	#[rustfmt::skip]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
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

// === Shortcuts === //

pub fn typed_key<T: ?Sized + 'static>() -> TypedKey<T> {
	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Static(NamedTypeId::of::<T>())),
	}
}

pub fn proxy_key<T: ?Sized + ProxyKeyType>() -> TypedKey<T::Provides> {
	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Proxy(NamedTypeId::of::<T>())),
	}
}

pub fn dyn_key<T: ?Sized + 'static>() -> TypedKey<T> {
	static GEN: AtomicU64 = AtomicU64::new(1);

	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Runtime(
			GEN.fetch_update(AtomicOrdering::Relaxed, AtomicOrdering::Relaxed, |gen| {
				Some(gen.checked_add(1).expect("allocated too many IDs"))
			})
			.unwrap(),
		)),
	}
}

#[doc(hidden)]
pub trait ProxyKeyType: 'static {
	type Provides: ?Sized + 'static;

	fn key() -> TypedKey<Self::Provides> {
		proxy_key::<Self>()
	}
}

pub macro proxy_key($(
	$(#[$macro_meta:meta])*
	$vis:vis struct $name:ident of $target:ty;
)*) {$(
	$(#[$macro_meta])*
	$vis struct $name;

	impl ProxyKeyType for $name {
		type Provides = $target;
	}
)*}
