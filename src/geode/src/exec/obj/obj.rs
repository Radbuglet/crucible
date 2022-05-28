use crate::exec::atomic_ref_cell::ARefCell;
use crate::exec::key::{typed_key, RawTypedKey, TypedKey};
use crate::exec::obj::{ProviderOut, RawObj};
use crate::util::arity_utils::impl_tuples;
use crate::util::inline_store::ByteContainer;
use crate::util::marker::{PhantomNoSendOrSync, PhantomNoSync};
use crate::util::usually::MakeSync;
use bumpalo::Bump;
use derive_where::derive_where;
use rustc_hash::FxHashMap;
use std::alloc::Layout;
use std::fmt::{Debug, Display, Formatter};
use std::marker::{PhantomData, Unsize};
use std::ptr::NonNull;

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

type InlinePtrContainer = ByteContainer<(usize, usize)>;

pub type StObj = Obj<SingleThreadedFlavor>;
pub type SendObj = Obj<SendFlavor>;

#[derive_where(Debug, Default)]
pub struct Obj<F: ObjFlavor = SendSyncFlavor> {
	flavor: PhantomData<F>,
	comps: FxHashMap<RawTypedKey, ObjEntry>,
	bump: MakeSync<Bump>,
	#[cfg(debug_assertions)]
	debug_label: Option<String>,
}

impl<F: ObjFlavor> Obj<F> {
	pub fn new() -> Self {
		Default::default()
	}

	#[allow(unused_variables)] // "name" is unused in release builds.
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
		assert!(
			!self.comps.contains_key(&owning_key),
			"Obj already contains component with key {owning_key:?}"
		);

		// Allocate component
		let comp = self.bump.get().alloc_layout(Layout::new::<T>()).cast::<T>();
		unsafe { comp.as_ptr().write(value) };

		// Register the principal entry
		let entry = ObjEntry::new_owned(self.bump.get(), comp);
		self.comps.insert(owning_key, entry);

		// Register alias entries
		unsafe { alias_as.register_aliases(self, comp) };
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
	fn provide_raw<'t, 'r>(&'r self, out: &mut ProviderOut<'t, 'r>) {
		let entry = match self.comps.get(&out.key()) {
			Some(entry) => entry,
			None => return,
		};

		let ptr = if InlinePtrContainer::can_host_dyn("dynamic", out.ptr_layout()).is_ok() {
			// Get pointer directly from the inline store
			entry.ptr.as_bytes_ptr()
		} else {
			// Get pointer from the bump
			*unsafe { entry.ptr.as_ref::<*const u8>() }
		};

		unsafe { out.provide_dynamic_unchecked(ptr) };
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
	ptr: InlinePtrContainer,
	drop_fn_or_alias: Option<unsafe fn(&mut ObjEntry)>,
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
	pub fn new_common<T: ?Sized>(bump: &mut Bump, ptr: NonNull<T>) -> InlinePtrContainer {
		if let Ok(inlined) = InlinePtrContainer::try_new(ptr) {
			inlined
		} else {
			// Reserve space on the bump.
			let ptr_on_heap = bump
				.alloc_layout(Layout::new::<NonNull<T>>())
				.cast::<NonNull<T>>();

			// And initialize it to the over-sized `ptr_meta`.
			unsafe { ptr_on_heap.as_ptr().write(ptr) }

			// Wrap the pointer to the heap.
			InlinePtrContainer::new::<*mut u8>(ptr_on_heap.as_ptr() as *mut u8)
		}
	}

	pub fn new_owned<T: Sized>(bump: &mut Bump, ptr: NonNull<T>) -> Self {
		let ptr = Self::new_common(bump, ptr);

		unsafe fn drop_ptr<T>(entry: &mut ObjEntry) {
			if InlinePtrContainer::can_host::<NonNull<T>>().is_ok() {
				entry.ptr.as_ref::<NonNull<T>>().as_ptr().drop_in_place();
			} else {
				// `p_ptr` is a pointer to the bytes of the target pointer.
				let p_ptr = *entry.ptr.as_ref::<*mut u8>();
				let p_ptr = p_ptr as *mut T;

				p_ptr.drop_in_place();
			}
		}

		let drop_fn: unsafe fn(&mut ObjEntry) = drop_ptr::<T>;

		Self {
			ptr,
			drop_fn_or_alias: Some(drop_fn),
			#[cfg(debug_assertions)]
			comp_name: std::any::type_name::<T>(),
		}
	}

	pub fn new_alias<T: ?Sized>(bump: &mut Bump, ptr: NonNull<T>) -> Self {
		let ptr = Self::new_common(bump, ptr);

		Self {
			ptr,
			drop_fn_or_alias: None,
			#[cfg(debug_assertions)]
			comp_name: std::any::type_name::<T>(),
		}
	}

	pub unsafe fn drop_if_owned(&mut self) {
		if let Some(drop_fn) = self.drop_fn_or_alias {
			drop_fn(self)
		}
	}
}

// === Alias lists === //

pub trait AliasList<T: Sized> {
	unsafe fn register_aliases<F: ObjFlavor>(self, map: &mut Obj<F>, ptr: NonNull<T>);
}

impl<T, U> AliasList<T> for TypedKey<U>
where
	T: Sized + Unsize<U>,
	U: ?Sized + 'static,
{
	unsafe fn register_aliases<F: ObjFlavor>(self, obj: &mut Obj<F>, ptr: NonNull<T>) {
		// Unsize the value and convert it back into a pointer
		let ptr = (ptr.as_ref() as &U) as *const U as *mut U;
		let ptr = NonNull::new_unchecked(ptr);

		// Insert the entry so long as it doesn't already exist
		let key = self.raw();
		assert!(
			!obj.comps.contains_key(&key),
			"Obj already contains component with key {key:?}",
		);

		let entry = ObjEntry::new_alias(obj.bump.get(), ptr);
		obj.comps.insert(key, entry);
	}
}

macro tup_impl_alias_list($($name:ident: $field:tt),*) {
	impl<_Src: Sized $(,$name: AliasList<_Src>)*> AliasList<_Src> for ($($name,)*) {
		#[allow(unused_variables)]
		unsafe fn register_aliases<F: ObjFlavor>(self, obj: &mut Obj<F>, ptr: NonNull<_Src>) {
			$( self.$field.register_aliases(obj, ptr); )*
		}
	}
}

impl_tuples!(tup_impl_alias_list);
