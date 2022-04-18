use std::any::TypeId;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};

/// A fancy [TypeId] that records type names in debug builds.
#[derive(Copy, Clone)]
pub struct FancyTypeId {
	id: TypeId,
	#[cfg(debug_assertions)]
	name: &'static str,
}

impl FancyTypeId {
	pub fn of<T: ?Sized + 'static>() -> Self {
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

impl Debug for FancyTypeId {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		#[cfg(debug_assertions)]
		{
			f.debug_tuple(format!("FancyTypeId<{}>", self.name).as_str())
				.field(&self.id)
				.finish()
		}
		#[cfg(not(debug_assertions))]
		{
			f.debug_tuple("FancyTypeId").field(&self.id).finish()
		}
	}
}

impl Hash for FancyTypeId {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.id.hash(state)
	}
}

impl Eq for FancyTypeId {}

impl PartialEq for FancyTypeId {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}
