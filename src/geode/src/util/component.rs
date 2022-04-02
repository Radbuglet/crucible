use crate::util::inline_store::ByteContainer;
use bumpalo::Bump;
use std::alloc::Layout;
use std::any::{type_name, TypeId};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ptr::{NonNull, Pointee};

/// A fancy [TypeId] that records type names on debug builds.
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
			name: type_name::<T>(),
		}
	}

	pub fn key(&self) -> TypeId {
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

pub struct Component {
	ptr: NonNull<()>,
	ptr_meta: ByteContainer<usize>,
	drop_fn_or_alias: Option<unsafe fn(*mut ())>,
	#[cfg(debug_assertions)]
	comp_name: &'static str,
}

impl Component {
	pub fn new_common<T: ?Sized>(
		ptr: NonNull<T>,
		bump: &mut Bump,
	) -> (NonNull<()>, ByteContainer<usize>) {
		let (ptr, ptr_meta) = ptr.to_raw_parts();
		let ptr_meta = if let Ok(inlined) = ByteContainer::<usize>::try_new(ptr_meta) {
			inlined
		} else {
			// Reserve space on the bump.
			let meta_on_heap = bump
				.alloc_layout(Layout::new::<<T as Pointee>::Metadata>())
				.cast::<<T as Pointee>::Metadata>();

			// And initialize it to the over-sized `ptr_meta`.
			unsafe { meta_on_heap.as_ptr().write(ptr_meta) }

			// Wrap the pointer to the heap.
			ByteContainer::<usize>::new(meta_on_heap)
		};

		(ptr, ptr_meta)
	}

	pub fn new_owned<T: Sized>(ptr: NonNull<T>, bump: &mut Bump) -> Self {
		let (ptr, ptr_meta) = Self::new_common(ptr, bump);

		unsafe fn drop_ptr<T>(ptr: *mut ()) {
			ptr.cast::<T>().drop_in_place()
		}

		let drop_fn: unsafe fn(*mut ()) = drop_ptr::<T>;

		Self {
			ptr,
			ptr_meta,
			drop_fn_or_alias: Some(drop_fn),
			#[cfg(debug_assertions)]
			comp_name: type_name::<T>(),
		}
	}

	pub fn new_alias<T: ?Sized>(ptr: NonNull<T>, bump: &mut Bump) -> Self {
		let (ptr, ptr_meta) = Self::new_common(ptr, bump);

		Self {
			ptr,
			ptr_meta,
			drop_fn_or_alias: None,
			#[cfg(debug_assertions)]
			comp_name: type_name::<T>(),
		}
	}

	pub unsafe fn target_ptr<T: ?Sized>(&self) -> NonNull<T> {
		let is_inline = ByteContainer::<usize>::can_host::<<T as Pointee>::Metadata>().is_ok();
		let ptr_meta = if is_inline {
			*self.ptr_meta.as_ref::<<T as Pointee>::Metadata>()
		} else {
			let ptr_to_meta = self.ptr_meta.as_ref::<NonNull<<T as Pointee>::Metadata>>();
			*ptr_to_meta.as_ref()
		};

		NonNull::from_raw_parts(self.ptr, ptr_meta)
	}

	pub unsafe fn drop_if_owned(&mut self) {
		if let Some(drop_fn) = self.drop_fn_or_alias {
			drop_fn(self.ptr.as_ptr())
		}
	}
}

impl Debug for Component {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let mut builder = f.debug_tuple("Component");
		#[cfg(debug_assertions)]
		builder.field(&self.comp_name);
		builder.finish()
	}
}
