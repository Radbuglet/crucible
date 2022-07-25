use std::{alloc::Layout, fmt, marker::PhantomData};

use crucible_core::type_id::NamedTypeId;

use super::session::Session;

#[derive(Copy, Clone)]
pub struct ReflectType {
	pub id: Option<NamedTypeId>,
	pub static_layout: Option<Layout>,
	pub drop_fn: Option<for<'a> unsafe fn(*mut (), Layout, Session)>,
}

impl fmt::Debug for ReflectType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("TypeMeta")
			.field("id", &self.id)
			.field("static_layout", &self.static_layout)
			.finish_non_exhaustive()
	}
}

impl ReflectType {
	pub const fn of<T: 'static>() -> &'static ReflectType {
		unsafe fn drop_raw_ptr<T>(value: *mut (), _layout: Layout, _session: Session) {
			std::ptr::drop_in_place(value as *mut T)
		}

		struct MetaProvider<T>(T);

		impl<T: 'static> MetaProvider<T> {
			const META: ReflectType = ReflectType {
				id: Some(NamedTypeId::of::<T>()),
				static_layout: Some(Layout::new::<T>()),
				drop_fn: if std::mem::needs_drop::<T>() {
					Some(drop_raw_ptr::<T>)
				} else {
					None
				},
			};
		}

		&MetaProvider::<T>::META
	}

	pub const fn dynamic_no_drop() -> &'static ReflectType {
		const INSTANCE: ReflectType = ReflectType {
			id: None,
			static_layout: None,
			drop_fn: None,
		};

		&INSTANCE
	}

	pub const fn dynamic_with_drop<D>() -> &'static ReflectType
	where
		// lol, trivially bypassed feature gate
		FeatureGateBypass<D>: CustomDropHandler,
	{
		struct MetaProvider<D: CustomDropHandler> {
			_ty: PhantomData<D>,
		}

		impl<D: CustomDropHandler> MetaProvider<D> {
			const META: ReflectType = ReflectType {
				id: None,
				static_layout: None,
				drop_fn: Some(D::destruct),
			};
		}

		&MetaProvider::<FeatureGateBypass<D>>::META
	}
}

pub trait CustomDropHandler {
	unsafe fn destruct(alloc_base: *mut (), layout: Layout, session: Session);
}

pub struct FeatureGateBypass<T>(T);

impl<T: CustomDropHandler> CustomDropHandler for FeatureGateBypass<T> {
	unsafe fn destruct(alloc_base: *mut (), layout: Layout, session: Session) {
		T::destruct(alloc_base, layout, session)
	}
}
