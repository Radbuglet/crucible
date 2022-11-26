use std::{
	any::{self, type_name, TypeId},
	borrow::Borrow,
	fmt, hash,
};

#[derive(Copy, Clone)]
pub struct NamedTypeId {
	id: any::TypeId,
	#[cfg(debug_assertions)]
	name: Option<&'static str>,
}

impl fmt::Debug for NamedTypeId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		#[cfg(debug_assertions)]
		if let Some(name) = self.name {
			return write!(f, "TypeId<{}>", name);
		}

		self.id.fmt(f)
	}
}

impl hash::Hash for NamedTypeId {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
	}
}

impl Eq for NamedTypeId {}

impl PartialEq for NamedTypeId {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

impl NamedTypeId {
	pub fn of<T: ?Sized + 'static>() -> Self {
		Self {
			id: any::TypeId::of::<T>(),
			#[cfg(debug_assertions)]
			name: Some(any::type_name::<T>()),
		}
	}

	pub fn from_raw(id: any::TypeId) -> Self {
		Self {
			id,
			#[cfg(debug_assertions)]
			name: None,
		}
	}

	pub fn raw(self) -> any::TypeId {
		self.id
	}
}

impl Borrow<TypeId> for NamedTypeId {
	fn borrow(&self) -> &TypeId {
		&self.id
	}
}

impl From<NamedTypeId> for TypeId {
	fn from(id: NamedTypeId) -> Self {
		id.raw()
	}
}

impl From<TypeId> for NamedTypeId {
	fn from(raw: TypeId) -> Self {
		Self::from_raw(raw)
	}
}

pub fn are_probably_equal<A: ?Sized, B: ?Sized>() -> bool {
	type_name::<A>() == type_name::<B>()
}
