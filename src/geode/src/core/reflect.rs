use std::{alloc::Layout, any::TypeId, fmt, hash, marker::PhantomData};

use super::session::Session;

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

#[derive(Copy, Clone)]
pub struct TypeMeta {
	pub id: Option<NamedTypeId>,
	pub layout: TypeMetaLayout,
	pub drop_fn: Option<for<'a> unsafe fn(*mut (), Layout, Session)>,
}

#[derive(Debug, Copy, Clone)]
pub enum TypeMetaLayout {
	Static(Layout),
	Dynamic,
}

impl fmt::Debug for TypeMeta {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("TypeMeta")
			.field("id", &self.id)
			.field("layout", &self.layout)
			.finish_non_exhaustive()
	}
}

impl TypeMeta {
	pub const fn of<T: 'static>() -> &'static TypeMeta {
		unsafe fn drop_raw_ptr<T>(value: *mut (), _layout: Layout, _session: Session) {
			std::ptr::drop_in_place(value as *mut T)
		}

		struct MetaProvider<T>(T);

		impl<T: 'static> MetaProvider<T> {
			const META: TypeMeta = TypeMeta {
				id: Some(NamedTypeId::of::<T>()),
				layout: TypeMetaLayout::Static(Layout::new::<T>()),
				drop_fn: if std::mem::needs_drop::<T>() {
					Some(drop_raw_ptr::<T>)
				} else {
					None
				},
			};
		}

		&MetaProvider::<T>::META
	}

	pub const fn dynamic_no_drop() -> &'static TypeMeta {
		const INSTANCE: TypeMeta = TypeMeta {
			id: None,
			layout: TypeMetaLayout::Dynamic,
			drop_fn: None,
		};

		&INSTANCE
	}

	pub const fn dynamic_with_drop<D>() -> &'static TypeMeta
	where
		// lol, trivially bypassed feature gate
		// (do we even need this? why the heck did I make this const-compatible in the first place?)
		(D,): CustomDropHandler,
	{
		struct MetaProvider<D: CustomDropHandler> {
			_ty: PhantomData<D>,
		}

		impl<D: CustomDropHandler> MetaProvider<D> {
			const META: TypeMeta = TypeMeta {
				id: None,
				layout: TypeMetaLayout::Dynamic,
				drop_fn: Some(D::destruct),
			};
		}

		&MetaProvider::<(D,)>::META
	}
}

pub trait CustomDropHandler {
	unsafe fn destruct(alloc_base: *mut (), layout: Layout, session: Session);
}

impl<T: CustomDropHandler> CustomDropHandler for (T,) {
	unsafe fn destruct(alloc_base: *mut (), layout: Layout, session: Session) {
		T::destruct(alloc_base, layout, session)
	}
}
