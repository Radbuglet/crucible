use std::{any::TypeId, borrow::Borrow, fmt};

#[derive(Copy, Clone)]
#[cfg_attr(not(debug_assertions), derive(Eq, PartialEq, Ord, PartialOrd, Hash))]
#[cfg_attr(
	debug_assertions,
	derive_where::derive_where(Eq, PartialEq, Ord, PartialOrd, Hash)
)]
pub struct NamedTypeId {
	id: TypeId,
	#[cfg(debug_assertions)]
	#[derive_where(skip)]
	name: Option<&'static str>,
}

impl fmt::Debug for NamedTypeId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		#[cfg(debug_assertions)]
		if let Some(name) = self.name {
			return write!(f, "TypeId<{name}>");
		}

		self.id.fmt(f)
	}
}

impl NamedTypeId {
	pub fn of<T: ?Sized + 'static>() -> Self {
		Self {
			id: TypeId::of::<T>(),
			#[cfg(debug_assertions)]
			name: Some(std::any::type_name::<T>()),
		}
	}

	pub fn from_raw(id: TypeId) -> Self {
		Self {
			id,
			#[cfg(debug_assertions)]
			name: None,
		}
	}

	pub fn raw(self) -> TypeId {
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
