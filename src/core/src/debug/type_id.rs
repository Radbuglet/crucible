use std::{any, borrow::Borrow, fmt, hash};

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

impl Borrow<any::TypeId> for NamedTypeId {
	fn borrow(&self) -> &any::TypeId {
		&self.id
	}
}

impl From<NamedTypeId> for any::TypeId {
	fn from(id: NamedTypeId) -> Self {
		id.raw()
	}
}

impl From<any::TypeId> for NamedTypeId {
	fn from(raw: any::TypeId) -> Self {
		Self::from_raw(raw)
	}
}
