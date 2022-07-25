use std::{
	alloc::{AllocError, Allocator, Layout},
	hash,
	ptr::NonNull,
};

use crate::core::{obj::RawObj, owned::Destructible, session::Session};

pub unsafe trait SmartAllocator {
	type Handle: Copy + hash::Hash + Eq;

	unsafe fn smart_deallocate_no_ctx(&self, handle: Self::Handle, layout: Layout);
}

pub unsafe trait SmartAllocatorFor<'c, C: 'c + Copy>: SmartAllocator {
	fn deref_handle(&self, context: C, handle: Self::Handle) -> NonNull<u8>;

	fn smart_allocate(
		&self,
		context: C,
		layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError>;

	unsafe fn smart_deallocate(&self, context: C, handle: Self::Handle, layout: Layout);

	fn smart_allocate_zeroed(
		&self,
		context: C,
		layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		let (handle, ptr) = self.smart_allocate(context, layout)?;
		unsafe { ptr.as_ptr().cast::<u8>().write_bytes(0, ptr.len()) }
		Ok((handle, ptr))
	}

	unsafe fn smart_grow(
		&self,
		context: C,
		old_handle: Self::Handle,
		old_ptr: NonNull<u8>,
		old_layout: Layout,
		new_layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		debug_assert!(new_layout.size() >= old_layout.size());

		let (new_handle, new_ptr) = self.smart_allocate(context, new_layout)?;

		old_ptr
			.as_ptr()
			.copy_to_nonoverlapping(new_ptr.as_ptr().cast::<u8>(), old_layout.size());

		self.smart_deallocate(context, old_handle, old_layout);

		Ok((new_handle, new_ptr))
	}

	unsafe fn smart_grow_zeroed(
		&self,
		context: C,
		old_handle: Self::Handle,
		old_ptr: NonNull<u8>,
		old_layout: Layout,
		new_layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		debug_assert!(new_layout.size() >= old_layout.size());

		let (new_handle, new_ptr) = self.smart_allocate_zeroed(context, new_layout)?;

		old_ptr
			.as_ptr()
			.copy_to_nonoverlapping(new_ptr.as_ptr().cast::<u8>(), old_layout.size());

		self.smart_deallocate(context, old_handle, old_layout);

		Ok((new_handle, new_ptr))
	}

	unsafe fn smart_shrink(
		&self,
		context: C,
		old_handle: Self::Handle,
		old_ptr: NonNull<u8>,
		old_layout: Layout,
		new_layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		debug_assert!(new_layout.size() <= old_layout.size());

		let (new_handle, new_ptr) = self.smart_allocate(context, new_layout)?;

		old_ptr
			.as_ptr()
			.copy_to_nonoverlapping(new_ptr.as_ptr().cast::<u8>(), new_layout.size());

		self.smart_deallocate(context, old_handle, old_layout);

		Ok((new_handle, new_ptr))
	}
}

unsafe impl<A: ?Sized + Allocator> SmartAllocator for A {
	type Handle = NonNull<u8>;

	unsafe fn smart_deallocate_no_ctx(&self, ptr: NonNull<u8>, layout: Layout) {
		self.deallocate(ptr, layout)
	}
}

unsafe impl<'c, A> SmartAllocatorFor<'c, ()> for A
where
	A: ?Sized + Allocator,
{
	fn deref_handle(&self, _: (), ptr: Self::Handle) -> NonNull<u8> {
		ptr
	}

	fn smart_allocate(
		&self,
		_context: (),
		layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		self.allocate(layout).map(|ptr| (ptr.cast(), ptr))
	}

	unsafe fn smart_deallocate(&self, _: (), ptr: Self::Handle, layout: Layout) {
		self.deallocate(ptr, layout)
	}

	fn smart_allocate_zeroed(
		&self,
		_context: (),
		layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		self.allocate_zeroed(layout).map(|ptr| (ptr.cast(), ptr))
	}

	unsafe fn smart_grow(
		&self,
		_context: (),
		old_handle: Self::Handle,
		_old_ptr: NonNull<u8>,
		old_layout: Layout,
		new_layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		self.grow(old_handle, old_layout, new_layout)
			.map(|ptr| (ptr.cast(), ptr))
	}

	unsafe fn smart_grow_zeroed(
		&self,
		_context: (),
		old_handle: Self::Handle,
		_old_ptr: NonNull<u8>,
		old_layout: Layout,
		new_layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		self.grow_zeroed(old_handle, old_layout, new_layout)
			.map(|ptr| (ptr.cast(), ptr))
	}

	unsafe fn smart_shrink(
		&self,
		_context: (),
		old_handle: Self::Handle,
		_old_ptr: NonNull<u8>,
		old_layout: Layout,
		new_layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		self.shrink(old_handle, old_layout, new_layout)
			.map(|ptr| (ptr.cast(), ptr))
	}
}

pub struct ObjAllocator;

unsafe impl SmartAllocator for ObjAllocator {
	type Handle = RawObj;

	unsafe fn smart_deallocate_no_ctx(&self, handle: Self::Handle, _layout: Layout) {
		handle.destruct();
	}
}

unsafe impl<'c> SmartAllocatorFor<'c, Session<'c>> for ObjAllocator {
	fn deref_handle(&self, session: Session<'c>, handle: Self::Handle) -> NonNull<u8> {
		handle.get_ptr(session)
	}

	fn smart_allocate(
		&self,
		session: Session<'c>,
		layout: Layout,
	) -> Result<(Self::Handle, NonNull<[u8]>), AllocError> {
		let (handle, ptr) = RawObj::new_dynamic(session, layout);
		Ok((
			handle.manually_destruct(),
			NonNull::from(unsafe {
				std::slice::from_raw_parts_mut(ptr.cast::<u8>(), layout.size())
			}),
		))
	}

	unsafe fn smart_deallocate(&self, session: Session<'c>, ptr: Self::Handle, _layout: Layout) {
		ptr.destroy(session);
	}
}
