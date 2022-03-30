use crate::exec::event::ObjectSafeEventTarget;
use crate::util::arity_utils::{impl_tuples, InjectableClosure};
use crate::util::error::ResultExt;
use crate::util::inline_store::ByteContainer;
use bumpalo::Bump;
use std::alloc::Layout;
use std::any::{type_name, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::marker::{PhantomData, Unsize};
use std::mem::replace;
use std::ops::{Deref, DerefMut};
use std::ptr::{NonNull, Pointee};
use std::rc::{Rc, Weak};
use thiserror::Error;

// === Obj core === //

#[derive(Debug, Clone, Default)]
pub struct Obj(Rc<RefCell<ObjInner>>);

#[derive(Default)]
pub struct ObjInner {
	parent: Option<Weak<RefCell<ObjInner>>>,
	comp_map: HashMap<TypeId, ObjEntry>,
	locks: Vec<LockCounter>,
	bump: Bump,
}

impl Obj {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn parent(&self) -> Option<Obj> {
		self.0
			.borrow()
			.parent
			.as_ref()
			.map(|parent| Obj(parent.upgrade().unwrap()))
	}

	pub fn set_parent(&self, parent: Option<&Obj>) {
		self.0.borrow_mut().parent = parent.map(|obj| Rc::downgrade(&obj.0));
	}

	pub fn ancestors(&self, include_self: bool) -> impl Iterator<Item = Obj> {
		let mut target_iter = if include_self {
			Some(self.clone())
		} else {
			self.parent()
		};

		std::iter::from_fn(move || {
			// Fetch target
			let target = target_iter.as_ref()?;

			// Get its parent
			let parent = target.parent();

			// Replace the iterator with the new parent and return the new element
			replace(&mut target_iter, parent)
		})
	}

	pub fn add<T: 'static>(&self, value: T) {
		self.add_as(value, ());
	}

	pub fn add_as<T: 'static, A: AliasList<T>>(&self, value: T, aliases: A) {
		let mut inner = self.0.borrow_mut();
		let inner = &mut *inner;

		// Allocate the value
		let ptr = inner.bump.alloc_layout(Layout::new::<T>()).cast::<T>();
		unsafe { ptr.as_ptr().write(value) }

		// Register it
		let lock_index = inner.locks.len();
		inner.locks.push(LockCounter::new());
		inner.comp_map.insert(
			TypeId::of::<T>(),
			ObjEntry::new_owned(ptr, lock_index, &mut inner.bump),
		);

		// Register bundle aliases
		unsafe {
			aliases.push_aliases(inner, ptr, lock_index);
		}
	}

	pub fn try_borrow_ref_here<T: ?Sized + 'static>(&self) -> Result<RwRef<T>, BorrowError> {
		let inner = self.0.borrow();

		// Try to fetch entry
		let entry =
			inner
				.comp_map
				.get(&TypeId::of::<T>())
				.ok_or(BorrowError::ComponentMissing {
					component_name: type_name::<T>(),
					error: ComponentMissingError,
				})?;

		// Try to acquire lock
		inner.locks[entry.lock_index]
			.try_lock_ref()
			.map_err(|error| BorrowError::LockError {
				component_name: type_name::<T>(),
				error,
			})?;

		// Construct a pointer to the target
		let comp = unsafe { entry.target_ptr::<T>() };

		// Create the reference guard
		Ok(RwRef {
			obj: self.clone(),
			lock_index: entry.lock_index,
			comp,
		})
	}

	pub fn try_borrow_mut_here<T: ?Sized + 'static>(&self) -> Result<RwMut<T>, BorrowError> {
		let inner = self.0.borrow();

		// Try to fetch entry
		let entry =
			inner
				.comp_map
				.get(&TypeId::of::<T>())
				.ok_or(BorrowError::ComponentMissing {
					component_name: type_name::<T>(),
					error: ComponentMissingError,
				})?;

		// Try to acquire lock
		inner.locks[entry.lock_index]
			.try_lock_mut()
			.map_err(|error| BorrowError::LockError {
				component_name: type_name::<T>(),
				error,
			})?;

		// Construct a pointer to the target
		let comp = unsafe { entry.target_ptr::<T>() };

		// Create the reference guard
		Ok(RwMut {
			obj: self.clone(),
			lock_index: entry.lock_index,
			comp,
		})
	}

	pub fn try_borrow_ref<T: ?Sized + 'static>(&self) -> Result<RwRef<T>, BorrowError> {
		for ancestor in self.ancestors(true) {
			match ancestor.try_borrow_ref_here::<T>() {
				Ok(comp) => return Ok(comp),
				Err(lock_error @ BorrowError::LockError { .. }) => return Err(lock_error),
				Err(BorrowError::ComponentMissing { .. }) => {}
			}
		}

		Err(BorrowError::ComponentMissing {
			component_name: type_name::<T>(),
			error: ComponentMissingError,
		})
	}

	pub fn try_borrow_mut<T: ?Sized + 'static>(&self) -> Result<RwMut<T>, BorrowError> {
		for ancestor in self.ancestors(true) {
			match ancestor.try_borrow_mut_here::<T>() {
				Ok(comp) => return Ok(comp),
				Err(lock_error @ BorrowError::LockError { .. }) => return Err(lock_error),
				Err(BorrowError::ComponentMissing { .. }) => {}
			}
		}

		Err(BorrowError::ComponentMissing {
			component_name: type_name::<T>(),
			error: ComponentMissingError,
		})
	}

	pub fn borrow_ref<T: ?Sized + 'static>(&self) -> RwRef<T> {
		self.try_borrow_ref::<T>().unwrap_pretty()
	}

	pub fn borrow_mut<T: ?Sized + 'static>(&self) -> RwMut<T> {
		self.try_borrow_mut::<T>().unwrap_pretty()
	}

	pub fn try_borrow_many<T: ObjBorrowable>(&self) -> Result<T, BorrowError> {
		T::try_borrow_from(self)
	}

	pub fn borrow_many<T: ObjBorrowable>(&self) -> T {
		self.try_borrow_many().unwrap_pretty()
	}

	pub fn inject<D, F>(&self, target: F) -> F::Return
	where
		D: ObjBorrowable,
		F: InjectableClosure<(), D>,
	{
		self.inject_with(target, ())
	}

	pub fn inject_with<A, D, F>(&self, mut target: F, args: A) -> F::Return
	where
		D: ObjBorrowable,
		F: InjectableClosure<A, D>,
	{
		target.call_injected(args, self.borrow_many())
	}

	// === Event handler extensions === //

	// TODO: allow non-'static events.
	pub fn add_event_handler<T, E, D>(&self, mut handler: T)
	where
		T: 'static + InjectableClosure<(E,), D>,
		E: 'static,
		D: ObjBorrowable,
	{
		self.add_as(
			move |event: ObjEvent<E>| {
				let obj = unsafe { event.obj.as_ref() };
				handler.call_injected((event.event,), obj.borrow_many());
			},
			(alias_as::<dyn ObjectSafeEventTarget<ObjEvent<E>>>(),),
		);
	}

	pub fn try_fire<E: 'static>(&self, event: E) -> Result<(), (E, ComponentMissingError)> {
		match self.try_borrow_mut::<dyn ObjectSafeEventTarget<ObjEvent<E>>>() {
			Ok(mut target) => {
				target.fire_but_object_safe(ObjEvent {
					obj: NonNull::from(self),
					event,
				});
				Ok(())
			}
			Err(BorrowError::LockError { error, .. }) => panic!("{}", error),
			Err(BorrowError::ComponentMissing { error, .. }) => Err((event, error)),
		}
	}

	pub fn fire<E: 'static>(&self, event: E) {
		self.try_fire(event).map_err(|(_, err)| err).unwrap_pretty()
	}
}

impl Debug for ObjInner {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		#[cfg(debug_assertions)]
		{
			let mut builder = f.debug_tuple("ObjInner");
			for entry in self.comp_map.values() {
				builder.field(&format_args!(
					"{}: {:?}",
					entry.comp_name,
					self.locks[entry.lock_index].state()
				));
			}
			builder.finish()
		}
		#[cfg(not(debug_assertions))]
		{
			f.debug_struct("Obj").finish_non_exhaustive()
		}
	}
}

impl Drop for ObjInner {
	fn drop(&mut self) {
		// Bump does not run destructors by itself.
		for comp in self.comp_map.values() {
			if let Some(drop_fn) = comp.drop_fn_or_alias {
				unsafe {
					(drop_fn)(comp.ptr.as_ptr());
				}
			}
		}
	}
}

// === Obj core internals === //

struct ObjEvent<E: 'static> {
	obj: NonNull<Obj>,
	event: E,
}

struct ObjEntry {
	ptr: NonNull<()>,
	ptr_meta: ByteContainer<usize>,
	lock_index: usize,
	drop_fn_or_alias: Option<unsafe fn(*mut ())>,
	#[cfg(debug_assertions)]
	comp_name: &'static str,
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
			#[cfg(debug_assertions)]
			comp_name: type_name::<T>(),
		}
	}

	fn new_alias<T: ?Sized>(ptr: NonNull<T>, lock_index: usize, bump: &mut Bump) -> Self {
		let (ptr, ptr_meta) = Self::new_common(ptr, bump);

		Self {
			ptr,
			ptr_meta,
			lock_index,
			drop_fn_or_alias: None,
			#[cfg(debug_assertions)]
			comp_name: type_name::<T>(),
		}
	}

	unsafe fn target_ptr<T: ?Sized>(&self) -> NonNull<T> {
		let is_inline = ByteContainer::<usize>::can_host::<<T as Pointee>::Metadata>().is_ok();
		let ptr_meta = if is_inline {
			*self.ptr_meta.as_ref::<<T as Pointee>::Metadata>()
		} else {
			let ptr_to_meta = self.ptr_meta.as_ref::<NonNull<<T as Pointee>::Metadata>>();
			*ptr_to_meta.as_ref()
		};

		NonNull::from_raw_parts(self.ptr, ptr_meta)
	}
}

#[derive(Debug, Clone)]
pub struct LockCounter(Cell<isize>);

impl Default for LockCounter {
	fn default() -> Self {
		Self(Cell::new(0))
	}
}

impl LockCounter {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn state(&self) -> LockState {
		LockState::decode(self.0.get())
	}

	pub fn try_lock_mut(&self) -> Result<(), LockError> {
		if self.0.get() != 0 {
			return Err(LockError::XorError(self.state()));
		}

		self.0.set(-1);
		Ok(())
	}

	pub fn try_lock_ref(&self) -> Result<(), LockError> {
		if self.0.get() < 0 {
			return Err(LockError::XorError(self.state()));
		}
		let new_count = self
			.0
			.get()
			.checked_add(1)
			.ok_or(LockError::TooManyImmutable)?;

		self.0.set(new_count);
		Ok(())
	}

	pub unsafe fn unlock_mut(&self) {
		debug_assert_eq!(self.0.get(), -1);
		self.0.set(0);
	}

	pub unsafe fn unlock_ref(&self) {
		debug_assert!(self.0.get() > 0);
		self.0.set(self.0.get() - 1);
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum LockState {
	Mutably,
	Immutably(usize),
	Unborrowed,
}

impl LockState {
	fn decode(count: isize) -> Self {
		if count == 0 {
			Self::Unborrowed
		} else if count > 0 {
			Self::Immutably(count as usize)
		} else {
			debug_assert_eq!(count, -1);
			Self::Mutably
		}
	}
}

// === Error types === //

// TODO: Make lock and component missing errors include target components.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
#[error("Component missing from `Obj`.")]
pub struct ComponentMissingError;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum LockError {
	XorError(LockState),
	TooManyImmutable,
}

impl Error for LockError {}

impl Display for LockError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::XorError(lock_state) => {
				f.write_str("Failed to lock component ")?;
				match lock_state {
					LockState::Mutably => {
						f.write_str(
							"immutably: 1 concurrent mutable borrow prevents shared immutable access.",
						)?;
					}
					LockState::Immutably(concurrent) => {
						write!(
							f,
							"mutably: {} concurrent immutable borrow{} prevent{} exclusive mutable access.",
							concurrent,
							// Gotta love English grammar
							if *concurrent == 1 { "" } else { "s" },
							if *concurrent == 1 { "s" } else { "" },
						)?;
					}
					LockState::Unborrowed => {
						#[cfg(debug_assertions)]
						unreachable!();
						#[cfg(not(debug_assertions))]
						f.write_str("even though it was unborrowed?!")?;
					}
				}
			}
			Self::TooManyImmutable => {
				write!(
					f,
					"Failed to lock component immutably: more than {} concurrent immutable borrows \
					 outstanding, which is the `isize::MAX` limit!",
					isize::MAX
				)?;
			}
		}
		Ok(())
	}
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
pub enum BorrowError {
	#[error("Error while borrowing component of type `{component_name}`. {error}")]
	ComponentMissing {
		component_name: &'static str,
		error: ComponentMissingError,
	},
	#[error("Error while borrowing component of type `{component_name}`. {error}")]
	LockError {
		component_name: &'static str,
		error: LockError,
	},
}

// === Multi-fetch === //

pub trait ObjBorrowable: Sized {
	fn try_borrow_from(obj: &Obj) -> Result<Self, BorrowError>;
}

impl ObjBorrowable for Obj {
	fn try_borrow_from(obj: &Obj) -> Result<Self, BorrowError> {
		Ok(obj.clone())
	}
}

impl<T: ?Sized + 'static> ObjBorrowable for RwMut<T> {
	fn try_borrow_from(obj: &Obj) -> Result<Self, BorrowError> {
		obj.try_borrow_mut()
	}
}

impl<T: ?Sized + 'static> ObjBorrowable for RwRef<T> {
	fn try_borrow_from(obj: &Obj) -> Result<Self, BorrowError> {
		obj.try_borrow_ref()
	}
}

macro impl_tup_obj_borrowable($($name:ident: $field:tt),*) {
	impl<$($name: ObjBorrowable),*> ObjBorrowable for ($($name,)*) {
		#[allow(unused_variables)]
		fn try_borrow_from(obj: &Obj) -> Result<Self, BorrowError> {
			Ok(($($name::try_borrow_from(obj)?,)*))
		}
	}
}

impl_tuples!(impl_tup_obj_borrowable);

// === Keys === //

pub unsafe trait AliasList<T> {
	unsafe fn push_aliases(self, obj: &mut ObjInner, ptr: NonNull<T>, lock_index: usize);
}

#[derive(Debug, Copy, Clone)]
pub struct AliasAs<T: ?Sized + 'static> {
	_ty: PhantomData<fn(T) -> T>,
}

pub fn alias_as<T: ?Sized + 'static>() -> AliasAs<T> {
	AliasAs { _ty: PhantomData }
}

unsafe impl<T: Unsize<U>, U: ?Sized + 'static> AliasList<T> for AliasAs<U> {
	unsafe fn push_aliases(self, obj: &mut ObjInner, ptr: NonNull<T>, lock_index: usize) {
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
		unsafe fn push_aliases(self, obj: &mut ObjInner, ptr: NonNull<ZZ>, lock_index: usize) {
			$( self.$field.push_aliases(obj, ptr, lock_index); )*
		}
	}
}

impl_tuples!(tup_impl_alias_list);

// === Rw Guards === //

#[derive(Debug)]
pub struct RwMut<T: ?Sized> {
	obj: Obj,
	lock_index: usize,
	// Safety: this reference is valid until `Obj` is dropped.
	comp: NonNull<T>,
}

impl<T: ?Sized> RwMut<T> {
	pub fn obj(ptr: &Self) -> &Obj {
		&ptr.obj
	}

	pub fn downgrade(ptr: Self) -> RwRef<T> {
		// Copy down guard state
		let obj = ptr.obj.clone();
		let lock_index = ptr.lock_index;
		let comp = ptr.comp;

		// Release guard
		drop(ptr);

		// Attempt to lock guard
		obj.0.borrow_mut().locks[lock_index]
			.try_lock_ref()
			.unwrap_pretty();

		// Create new guard
		RwRef {
			obj,
			lock_index,
			comp,
		}
	}
}

impl<T: ?Sized> Deref for RwMut<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { self.comp.as_ref() }
	}
}

impl<T: ?Sized> DerefMut for RwMut<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { self.comp.as_mut() }
	}
}

impl<T: ?Sized> Drop for RwMut<T> {
	fn drop(&mut self) {
		unsafe {
			self.obj.0.borrow_mut().locks[self.lock_index].unlock_mut();
		}
	}
}

#[derive(Debug)]
pub struct RwRef<T: ?Sized> {
	obj: Obj,
	lock_index: usize,
	// Safety: this reference is valid until `Obj` is dropped.
	comp: NonNull<T>,
}

impl<T: ?Sized> RwRef<T> {
	pub fn obj(ptr: &Self) -> &Obj {
		&ptr.obj
	}

	pub fn upgrade(ptr: Self) -> RwMut<T> {
		// Copy down guard state
		let obj = ptr.obj.clone();
		let lock_index = ptr.lock_index;
		let comp = ptr.comp;

		// Release guard
		drop(ptr);

		// Attempt to lock guard
		obj.0.borrow_mut().locks[lock_index]
			.try_lock_mut()
			.unwrap_pretty();

		// Create new guard
		RwMut {
			obj,
			lock_index,
			comp,
		}
	}
}

impl<T: ?Sized> Deref for RwRef<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { self.comp.as_ref() }
	}
}

impl<T: ?Sized> Clone for RwRef<T> {
	fn clone(&self) -> Self {
		self.obj.0.borrow_mut().locks[self.lock_index]
			.try_lock_ref()
			.unwrap_pretty();

		Self {
			obj: self.obj.clone(),
			lock_index: self.lock_index,
			comp: self.comp,
		}
	}
}

impl<T: ?Sized> Drop for RwRef<T> {
	fn drop(&mut self) {
		unsafe {
			self.obj.0.borrow_mut().locks[self.lock_index].unlock_ref();
		}
	}
}
