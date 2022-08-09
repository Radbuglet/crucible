use core::{any::TypeId, fmt, hash};

/// A fancy [TypeId] that records type names in debug builds.
#[derive(Copy, Clone)]
pub struct NamedTypeId {
	id: TypeId,
	#[cfg(debug_assertions)]
	name: &'static str,
}

impl NamedTypeId {
	pub const fn of<T: ?Sized + 'static>() -> Self {
		Self {
			id: TypeId::of::<T>(),
			#[cfg(debug_assertions)]
			name: std::any::type_name::<T>(),
		}
	}

	pub fn raw(&self) -> TypeId {
		self.id
	}

	pub fn name(&self) -> &'static str {
		#[cfg(debug_assertions)]
		{
			self.name
		}
		#[cfg(not(debug_assertions))]
		{
			"type name unavailable"
		}
	}
}

impl fmt::Debug for NamedTypeId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		#[cfg(debug_assertions)]
		{
			f.debug_tuple(format!("NamedTypeId<{}>", self.name).as_str())
				.finish()
		}
		#[cfg(not(debug_assertions))]
		{
			f.debug_tuple("NamedTypeId").field(&self.id).finish()
		}
	}
}

impl hash::Hash for NamedTypeId {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state)
	}
}

impl Eq for NamedTypeId {}

impl PartialEq for NamedTypeId {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}
