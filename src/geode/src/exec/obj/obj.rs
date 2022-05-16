use crate::exec::atomic_ref_cell::ARefCell;
use crate::exec::key::{typed_key, RawTypedKey, TypedKey};
use crate::exec::obj::{ProviderOut, RawObj};
use crate::util::arity_utils::impl_tuples;
use crate::util::inline_store::ByteContainer;
use crate::util::marker::{PhantomNoSendOrSync, PhantomNoSync};
use crate::util::usually::MakeSync;
use bumpalo::Bump;
use derive_where::derive_where;
use std::alloc::Layout;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::marker::{PhantomData, Unsize};
use std::ptr::{NonNull, Pointee};

// === Flavor definitions === //

pub trait ObjFlavor: Sized + sealed::Sealed {}

pub unsafe trait ObjFlavorCanOwn<T: ?Sized>: ObjFlavor {}

pub struct SendSyncFlavor {
	_private: (),
}

pub struct SendFlavor {
	_private: PhantomNoSync,
}

pub struct SingleThreadedFlavor {
	_private: PhantomNoSendOrSync,
}

mod sealed {
	use super::*;

	pub trait Sealed {}

	// SendSyncFlavor
	impl Sealed for SendSyncFlavor {}
	impl ObjFlavor for SendSyncFlavor {}
	unsafe impl<T: ?Sized + Send + Sync> ObjFlavorCanOwn<T> for SendSyncFlavor {}

	// SendFlavor
	impl Sealed for SendFlavor {}
	impl ObjFlavor for SendFlavor {}
	unsafe impl<T: ?Sized + Send> ObjFlavorCanOwn<T> for SendFlavor {}

	// SingleThreadedFlavor
	impl Sealed for SingleThreadedFlavor {}
	impl ObjFlavor for SingleThreadedFlavor {}
	unsafe impl<T: ?Sized> ObjFlavorCanOwn<T> for SingleThreadedFlavor {}
}

// === Obj === //

type InlineMetaContainer = ByteContainer<usize>;
type HeapMetaPtr = NonNull<u8>;

pub type StObj = Obj<SingleThreadedFlavor>;
pub type SendObj = Obj<SendFlavor>;

#[derive_where(Debug, Default)]
pub struct Obj<F: ObjFlavor = SendSyncFlavor> {
	flavor: PhantomData<F>,
	comps: HashMap<RawTypedKey, ObjEntry>,
	bump: MakeSync<Bump>,
	#[cfg(debug_assertions)]
	debug_label: Option<String>,
}

impl<F: ObjFlavor> Obj<F> {
	pub fn new() -> Self {
		Default::default()
	}

	#[allow(unused_variables)] // For "name" in release builds.
	pub fn labeled<D: Display>(name: D) -> Self {
		Self {
			flavor: PhantomData,
			comps: Default::default(),
			bump: Default::default(),
			#[cfg(debug_assertions)]
			debug_label: Some(name.to_string()),
		}
	}

	pub fn debug_label(&self) -> &str {
		#[cfg(debug_assertions)]
		{
			self.debug_label.as_ref().map_or("unset", String::as_str)
		}
		#[cfg(not(debug_assertions))]
		{
			"unavailable"
		}
	}

	pub fn add_as<T, A>(&mut self, value: T, owning_key: TypedKey<T>, alias_as: A)
	where
		F: ObjFlavorCanOwn<T>,
		A: AliasList<T>,
	{
		// Ensure that we haven't already registered this key.
		let owning_key = owning_key.raw();
		assert!(!self.comps.contains_key(&owning_key));

		// Allocate component
		let comp = self.bump.get().alloc_layout(Layout::new::<T>()).cast::<T>();
		unsafe {
			comp.as_ptr().write(value);
		}

		// Register the principal entry
		let entry = ObjEntry::new_owned(comp, self.bump.get());
		self.comps.insert(owning_key, entry);

		// Register alias entries
		unsafe {
			alias_as.push_aliases(self, comp);
		}
	}

	pub fn add_in<T>(&mut self, value: T, owning_key: TypedKey<T>)
	where
		F: ObjFlavorCanOwn<T>,
	{
		self.add_as(value, owning_key, ());
	}

	pub fn add<T: 'static>(&mut self, value: T)
	where
		F: ObjFlavorCanOwn<T>,
	{
		self.add_in(value, typed_key::<T>());
	}

	pub fn add_alias<T: 'static, A>(&mut self, value: T, alias_as: A)
	where
		F: ObjFlavorCanOwn<T>,
		A: AliasList<T>,
	{
		self.add_as(value, typed_key(), alias_as);
	}

	pub fn add_rw<T: 'static>(&mut self, value: T)
	where
		F: ObjFlavorCanOwn<ARefCell<T>>,
	{
		self.add(ARefCell::new(value));
	}
}

impl<F: ObjFlavor> RawObj for Obj<F> {
	fn provide_raw<'r>(&'r self, out: &mut ProviderOut<'r>) {
		let entry = match self.comps.get(&out.key()) {
			Some(entry) => entry,
			None => return,
		};

		let p_meta = if InlineMetaContainer::can_host_dyn("dynamic", out.meta_layout()).is_ok() {
			// Get meta directly from the inline store
			entry.ptr_meta.as_bytes_ptr()
		} else {
			// Get meta from the bump
			unsafe { entry.ptr_meta.as_ref::<HeapMetaPtr>() }.as_ptr()
		};

		unsafe { out.provide_dynamic_unchecked(entry.base, p_meta) };
	}
}

impl<F: ObjFlavor> Drop for Obj<F> {
	fn drop(&mut self) {
		for comp in self.comps.values_mut() {
			unsafe {
				comp.drop_if_owned();
			}
		}
	}
}

struct ObjEntry {
	base: NonNull<()>,
	ptr_meta: InlineMetaContainer,
	drop_fn_or_alias: Option<unsafe fn(*mut ())>,
	#[cfg(debug_assertions)]
	comp_name: &'static str,
}

impl Debug for ObjEntry {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let mut builder = f.debug_tuple("ObjEntry");
		#[cfg(debug_assertions)]
		builder.field(&self.comp_name);
		builder.finish()
	}
}

// Unsound methods exposing the contents of the `ObjEntry` are all `unsafe`.
unsafe impl Send for ObjEntry {}
unsafe impl Sync for ObjEntry {}

impl ObjEntry {
	pub fn new_common<T: ?Sized>(
		ptr: NonNull<T>,
		bump: &mut Bump,
	) -> (NonNull<()>, InlineMetaContainer) {
		let (ptr, ptr_meta) = ptr.to_raw_parts();
		let ptr_meta = if let Ok(inlined) = InlineMetaContainer::try_new(ptr_meta) {
			inlined
		} else {
			// Reserve space on the bump.
			let meta_on_heap = bump
				.alloc_layout(Layout::new::<<T as Pointee>::Metadata>())
				.cast::<<T as Pointee>::Metadata>();

			// And initialize it to the over-sized `ptr_meta`.
			unsafe { meta_on_heap.as_ptr().write(ptr_meta) }

			// Wrap the pointer to the heap.
			ByteContainer::<usize>::new::<HeapMetaPtr>(meta_on_heap.cast::<u8>())
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
			base: ptr,
			ptr_meta,
			drop_fn_or_alias: Some(drop_fn),
			#[cfg(debug_assertions)]
			comp_name: std::any::type_name::<T>(),
		}
	}

	pub fn new_alias<T: ?Sized>(ptr: NonNull<T>, bump: &mut Bump) -> Self {
		let (ptr, ptr_meta) = Self::new_common(ptr, bump);

		Self {
			base: ptr,
			ptr_meta,
			drop_fn_or_alias: None,
			#[cfg(debug_assertions)]
			comp_name: std::any::type_name::<T>(),
		}
	}

	pub unsafe fn drop_if_owned(&mut self) {
		if let Some(drop_fn) = self.drop_fn_or_alias {
			drop_fn(self.base.as_ptr())
		}
	}
}

// === Alias lists === //

pub unsafe trait AliasList<T: Sized> {
	unsafe fn push_aliases<F: ObjFlavor>(self, map: &mut Obj<F>, ptr: NonNull<T>);
}

unsafe impl<T, U> AliasList<T> for TypedKey<U>
where
	T: Sized + Unsize<U>,
	U: ?Sized + 'static,
{
	unsafe fn push_aliases<F: ObjFlavor>(self, map: &mut Obj<F>, ptr: NonNull<T>) {
		// Unsize the value and convert it back into a pointer
		let ptr = (ptr.as_ref() as &U) as *const U as *mut U;
		let ptr = NonNull::new_unchecked(ptr);

		// Insert the entry
		#[rustfmt::skip]
		map.comps.insert(
			self.raw(),
			ObjEntry::new_alias(ptr, map.bump.get())
		);
	}
}

macro tup_impl_alias_list($($name:ident: $field:tt),*) {
unsafe impl<_Src: Sized $(,$name: AliasList<_Src>)*> AliasList<_Src> for ($($name,)*) {
		#[allow(unused_variables)]
		unsafe fn push_aliases<F: ObjFlavor>(self, obj: &mut Obj<F>, ptr: NonNull<_Src>) {
			$( self.$field.push_aliases(obj, ptr); )*
		}
	}
}

impl_tuples!(tup_impl_alias_list);
