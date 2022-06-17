use std::alloc::Layout;
use std::any::TypeId;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};

/// A fancy [TypeId] that records type names in debug builds.
#[derive(Copy, Clone)]
pub struct NamedTypeId {
	id: TypeId,
	#[cfg(debug_assertions)]
	name: &'static str,
}

impl NamedTypeId {
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

impl Debug for NamedTypeId {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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

impl Hash for NamedTypeId {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.id.hash(state)
	}
}

impl Eq for NamedTypeId {}

impl PartialEq for NamedTypeId {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

#[derive(Debug, Copy, Clone)]
pub struct TypeMeta {
	pub layout: Layout,
	pub drop_fn: Option<unsafe fn(*mut ())>,
}

impl TypeMeta {
	pub fn of<T>() -> &'static TypeMeta {
		unsafe fn drop_raw_ptr<T>(value: *mut ()) {
			std::ptr::drop_in_place(value as *mut T)
		}

		struct MetaProvider<T>(T);

		impl<T> MetaProvider<T> {
			const META: TypeMeta = TypeMeta {
				layout: Layout::new::<T>(),
				drop_fn: if std::mem::needs_drop::<T>() {
					Some(drop_raw_ptr::<T>)
				} else {
					None
				},
			};
		}

		&MetaProvider::<T>::META
	}
}
