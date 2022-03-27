use crate::exec::event::ObjectSafeEventTarget;
use crate::util::inline_store::ByteContainer;
use crate::util::macro_util::impl_tuples;
use bumpalo::Bump;
use std::alloc::Layout;
use std::any::TypeId;
use std::cell::Cell;
use std::collections::HashMap;
use std::marker::{PhantomData, Unsize};
use std::ptr::{NonNull, Pointee};

#[derive(Default)]
pub struct Obj {
	comp_map: HashMap<TypeId, ObjEntry>,
	locks: Vec<Cell<isize>>,
	bump: Bump,
}

impl Obj {
	pub fn add<T: 'static>(&mut self, value: T) {
		self.add_as(value, ());
	}

	pub fn add_as<T: 'static, A: AliasList<T>>(&mut self, value: T, aliases: A) {
		// Allocate the value
		let ptr = self.bump.alloc_layout(Layout::new::<T>()).cast::<T>();
		unsafe { ptr.as_ptr().write(value) }

		// Register it
		let lock_index = self.locks.len();
		self.locks.push(Cell::new(0));
		self.comp_map.insert(
			TypeId::of::<T>(),
			ObjEntry::new_owned(ptr, lock_index, &mut self.bump),
		);

		// Register bundle aliases
		unsafe {
			aliases.push_aliases(self, ptr, lock_index);
		}
	}

	pub fn add_event_handler<T, E>(&mut self, handler: T)
	where
		T: 'static + ObjectSafeEventTarget<E>,
		E: 'static,
	{
		self.add_as(handler, (alias_as::<dyn ObjectSafeEventTarget<E>>(),));
	}

	pub fn try_get_raw<T: ?Sized + 'static>(&self) -> Option<NonNull<T>> {
		self.comp_map.get(&TypeId::of::<T>()).map(|entry| {
			let is_inline = ByteContainer::<usize>::can_host::<<T as Pointee>::Metadata>().is_ok();
			let ptr_meta = if is_inline {
				unsafe { *entry.ptr_meta.as_ref::<<T as Pointee>::Metadata>() }
			} else {
				unsafe {
					let ptr_to_meta = entry.ptr_meta.as_ref::<NonNull<<T as Pointee>::Metadata>>();
					*ptr_to_meta.as_ref()
				}
			};

			NonNull::from_raw_parts(entry.ptr, ptr_meta)
		})
	}

	pub fn try_get<T: ?Sized + 'static>(&self) -> Option<&'_ T> {
		self.try_get_raw().map(|inner| unsafe { inner.as_ref() })
	}

	pub fn try_get_mut<T: ?Sized + 'static>(&mut self) -> Option<&'_ mut T> {
		self.try_get_raw()
			.map(|mut inner| unsafe { inner.as_mut() })
	}
}

impl Drop for Obj {
	fn drop(&mut self) {
		for comp in self.comp_map.values() {
			if let Some(drop_fn) = comp.drop_fn_or_alias {
				unsafe {
					(drop_fn)(comp.ptr.as_ptr());
				}
			}
		}
	}
}

struct ObjEntry {
	ptr: NonNull<()>,
	ptr_meta: ByteContainer<usize>,
	lock_index: usize,
	drop_fn_or_alias: Option<unsafe fn(*mut ())>,
}

impl ObjEntry {
	fn new_common<T: ?Sized>(
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

	fn new_owned<T>(ptr: NonNull<T>, lock_index: usize, bump: &mut Bump) -> Self {
		let (ptr, ptr_meta) = Self::new_common(ptr, bump);

		unsafe fn drop_ptr<T>(ptr: *mut ()) {
			ptr.cast::<T>().drop_in_place()
		}

		let drop_fn: unsafe fn(*mut ()) = drop_ptr::<T>;

		Self {
			ptr,
			ptr_meta,
			lock_index,
			drop_fn_or_alias: Some(drop_fn),
		}
	}

	fn new_alias<T: ?Sized>(ptr: NonNull<T>, lock_index: usize, bump: &mut Bump) -> Self {
		let (ptr, ptr_meta) = Self::new_common(ptr, bump);

		Self {
			ptr,
			ptr_meta,
			lock_index,
			drop_fn_or_alias: None,
		}
	}
}

pub unsafe trait AliasList<T> {
	unsafe fn push_aliases(self, obj: &mut Obj, ptr: NonNull<T>, lock_index: usize);
}

#[derive(Debug, Copy, Clone)]
pub struct AliasAs<T: ?Sized + 'static> {
	_ty: PhantomData<fn(T) -> T>,
}

pub fn alias_as<T: ?Sized + 'static>() -> AliasAs<T> {
	AliasAs { _ty: PhantomData }
}

unsafe impl<T: Unsize<U>, U: ?Sized + 'static> AliasList<T> for AliasAs<U> {
	unsafe fn push_aliases(self, obj: &mut Obj, ptr: NonNull<T>, lock_index: usize) {
		// Unsize the value and convert it back into a pointer
		let ptr = (ptr.as_ref() as &U) as *const U as *mut U;
		let ptr = NonNull::new_unchecked(ptr);

		// Insert the entry
		obj.comp_map.insert(
			TypeId::of::<U>(),
			ObjEntry::new_alias(ptr, lock_index, &mut obj.bump),
		);
	}
}

macro tup_impl_alias_list($($name:ident: $field:tt),*) {
	unsafe impl<ZZ $(,$name: AliasList<ZZ>)*> AliasList<ZZ> for ($($name,)*) {
		#[allow(unused_variables)]
		unsafe fn push_aliases(self, obj: &mut Obj, ptr: NonNull<ZZ>, lock_index: usize) {
			$( self.$field.push_aliases(obj, ptr, lock_index); )*
		}
	}
}

impl_tuples!(tup_impl_alias_list);
