use crate::exec::atomic_ref_cell::{AMut, ARef, ARefCell, LockError};
use crate::util::arity_utils::{impl_tuples, InjectableClosure};
use crate::util::error::ResultExt;
use crate::util::inline_store::ByteContainer;
use crate::util::number::NumberGenRef;
use crate::util::type_id::FancyTypeId;
use bumpalo::Bump;
use derive_where::derive_where;
use std::alloc::Layout;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;
use std::marker::{PhantomData, Unsize};
use std::ptr::{NonNull, Pointee};
use std::sync::atomic::AtomicU64;
use thiserror::Error;

// === Obj core === //

pub trait ComponentValue: Sized + 'static + Send + Sync {}

impl<T: 'static + Send + Sync> ComponentValue for T {}

pub unsafe trait ObjLike {
	// === Basic getters === //

	fn try_get_raw<T: ?Sized + 'static>(
		&self,
		key: TypedKey<T>,
	) -> Result<NonNull<T>, ComponentMissingError>;

	fn get_raw<T: ?Sized + 'static>(&self, key: TypedKey<T>) -> NonNull<T> {
		self.try_get_raw(key).unwrap_pretty()
	}

	fn try_get_as<T: ?Sized + 'static>(
		&self,
		key: TypedKey<T>,
	) -> Result<&T, ComponentMissingError> {
		self.try_get_raw(key).map(|value| unsafe { value.as_ref() })
	}

	fn get_as<T: ?Sized + 'static>(&self, key: TypedKey<T>) -> &T {
		self.try_get_as(key).unwrap_pretty()
	}

	fn try_get<T: ?Sized + 'static>(&self) -> Result<&T, ComponentMissingError> {
		self.try_get_as(typed_key::<T>())
	}

	fn get<T: ?Sized + 'static>(&self) -> &T {
		self.try_get().unwrap_pretty()
	}

	// === Borrow getters === //

	fn try_borrow_as<T: ?Sized + 'static>(
		&self,
		key: TypedKey<ARefCell<T>>,
	) -> Result<ARef<T>, BorrowError> {
		Ok(self
			.try_get_as(key)?
			.try_borrow()
			.map_err(|error| ComponentLockError {
				key: key.raw(),
				error,
			})?)
	}

	fn borrow_as<T: ?Sized + 'static>(&self, key: TypedKey<ARefCell<T>>) -> ARef<T> {
		self.try_borrow_as(key).unwrap_pretty()
	}

	fn try_borrow_mut_as<T: ?Sized + 'static>(
		&self,
		key: TypedKey<ARefCell<T>>,
	) -> Result<AMut<T>, BorrowError> {
		Ok(self
			.try_get_as(key)?
			.try_borrow_mut()
			.map_err(|error| ComponentLockError {
				key: key.raw(),
				error,
			})?)
	}

	fn borrow_mut_as<T: ?Sized + 'static>(&self, key: TypedKey<ARefCell<T>>) -> AMut<T> {
		self.try_borrow_mut_as(key).unwrap_pretty()
	}

	fn try_borrow<T: ?Sized + 'static>(&self) -> Result<ARef<T>, BorrowError> {
		self.try_borrow_as(typed_key::<ARefCell<T>>())
	}

	fn borrow<T: ?Sized + 'static>(&self) -> ARef<T> {
		self.try_borrow::<T>().unwrap_pretty()
	}

	fn try_borrow_mut<T: ?Sized + 'static>(&self) -> Result<AMut<T>, BorrowError> {
		self.try_borrow_mut_as(typed_key::<ARefCell<T>>())
	}

	fn borrow_mut<T: ?Sized + 'static>(&self) -> AMut<T> {
		self.try_borrow_mut::<T>().unwrap_pretty()
	}

	// === Multi-getters === //

	fn try_borrow_many<'a, D: MultiBorrowTarget<'a>>(&'a self) -> Result<D, BorrowError> {
		D::try_borrow_from(self)
	}

	fn borrow_many<'a, D: MultiBorrowTarget<'a>>(&'a self) -> D {
		self.try_borrow_many().unwrap_pretty()
	}

	fn inject<'a, D, F>(&'a self, mut handler: F) -> F::Return
	where
		D: MultiBorrowTarget<'a>,
		F: InjectableClosure<(), D>,
	{
		handler.call_injected((), self.borrow_many())
	}

	fn inject_with<'a, A, D, F>(&'a self, mut handler: F, args: A) -> F::Return
	where
		D: MultiBorrowTarget<'a>,
		F: InjectableClosure<A, D>,
	{
		handler.call_injected(args, self.borrow_many())
	}
}

#[derive(Debug, Default)]
pub struct Obj {
	comps: HashMap<RawTypedKey, ObjEntry>,
	bump: Bump,
	#[cfg(debug_assertions)]
	debug_label: Option<String>,
}

// `Obj` is `Send` and `Sync` because all components inserted into it must also be `Send` and `Sync`.
unsafe impl Send for Obj {}
unsafe impl Sync for Obj {}

impl Obj {
	pub fn new() -> Self {
		Default::default()
	}

	#[allow(unused_variables)] // For "name" in release builds.
	pub fn labeled<D: Display>(name: D) -> Self {
		Self {
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

	pub fn add<T: ComponentValue>(&mut self, value: T) {
		self.add_as(typed_key::<T>(), value, ());
	}

	pub fn add_as<T, A>(&mut self, owning_key: TypedKey<T>, value: T, alias_as: A)
	where
		T: ComponentValue,
		A: AliasList<T>,
	{
		// Ensure that we haven't already registered this key.
		let owning_key = owning_key.raw();
		assert!(!self.comps.contains_key(&owning_key));

		// Allocate component
		let comp = self.bump.alloc_layout(Layout::new::<T>()).cast::<T>();
		unsafe {
			comp.as_ptr().write(value);
		}

		// Register the principal entry
		#[rustfmt::skip]
		self.comps.insert(owning_key, ObjEntry::new_owned(comp, &mut self.bump));

		// Register alias entries
		unsafe {
			alias_as.push_aliases(self, comp);
		}
	}

	pub fn add_alias<T, A>(&mut self, value: T, alias_as: A)
	where
		T: ComponentValue,
		A: AliasList<T>,
	{
		self.add_as(typed_key(), value, alias_as);
	}

	pub fn add_rw<T: ComponentValue>(&mut self, value: T) {
		self.add(ARefCell::new(value));
	}

	// TODO: Single-threaded accessor wrapper
	// TODO: Integration with storages
}

unsafe impl ObjLike for Obj {
	fn try_get_raw<T: ?Sized + 'static>(
		&self,
		key: TypedKey<T>,
	) -> Result<NonNull<T>, ComponentMissingError> {
		let entry = self
			.comps
			.get(&key.raw())
			.ok_or(ComponentMissingError { key: key.raw() })?;

		Ok(unsafe { entry.target_ptr::<T>() })
	}
}

impl Drop for Obj {
	fn drop(&mut self) {
		for comp in self.comps.values_mut() {
			unsafe {
				comp.drop_if_owned();
			}
		}
	}
}

struct ObjEntry {
	ptr: NonNull<()>,
	ptr_meta: ByteContainer<usize>,
	drop_fn_or_alias: Option<unsafe fn(*mut ())>,
	#[cfg(debug_assertions)]
	comp_name: &'static str,
}

impl ObjEntry {
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
			comp_name: std::any::type_name::<T>(),
		}
	}

	pub fn new_alias<T: ?Sized>(ptr: NonNull<T>, bump: &mut Bump) -> Self {
		let (ptr, ptr_meta) = Self::new_common(ptr, bump);

		Self {
			ptr,
			ptr_meta,
			drop_fn_or_alias: None,
			#[cfg(debug_assertions)]
			comp_name: std::any::type_name::<T>(),
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

impl Debug for ObjEntry {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let mut builder = f.debug_tuple("ObjEntry");
		#[cfg(debug_assertions)]
		builder.field(&self.comp_name);
		builder.finish()
	}
}

// === ObjCx === //

pub struct ObjCx<'borrow, 'obj> {
	backing: ObjCxBacking<'borrow, 'obj>,
	length: usize,
}

enum ObjCxBacking<'borrow, 'obj> {
	Root(Vec<&'obj Obj>),
	Child(&'borrow mut Vec<&'obj Obj>),
}

impl<'borrow, 'obj> Debug for ObjCx<'borrow, 'obj> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ObjCx")
			.field("children", &self.path())
			.finish_non_exhaustive()
	}
}

impl<'obj> ObjCx<'_, 'obj> {
	pub fn root(root: &'obj Obj) -> Self {
		Self {
			backing: ObjCxBacking::Root(vec![root]),
			length: 1,
		}
	}

	fn backing_ref(&self) -> &Vec<&'obj Obj> {
		match &self.backing {
			ObjCxBacking::Root(root) => root,
			ObjCxBacking::Child(root) => *root,
		}
	}

	pub fn path(&self) -> &[&'obj Obj] {
		&self.backing_ref()[0..self.length]
	}

	pub fn ancestors(&self, include_self: bool) -> impl Iterator<Item = &'obj Obj> + '_ {
		let path = self.path();
		let path = if include_self {
			path
		} else {
			&path[0..path.len()]
		};

		path.iter().copied().rev()
	}

	pub fn me(&self) -> &'obj Obj {
		self.path().last().unwrap()
	}

	pub fn add<'borrow>(&'borrow mut self, child: &'obj Obj) -> ObjCx<'borrow, 'obj> {
		let root = match &mut self.backing {
			ObjCxBacking::Root(root) => root,
			ObjCxBacking::Child(root) => *root,
		};

		root.truncate(self.length);
		root.push(child);

		ObjCx {
			backing: ObjCxBacking::Child(root),
			length: self.length + 1,
		}
	}

	pub fn clone(&self) -> ObjCx<'static, 'obj> {
		ObjCx {
			backing: ObjCxBacking::Root(self.backing_ref().clone()),
			length: self.length,
		}
	}
}

unsafe impl ObjLike for ObjCx<'_, '_> {
	fn try_get_raw<T: ?Sized + 'static>(
		&self,
		key: TypedKey<T>,
	) -> Result<NonNull<T>, ComponentMissingError> {
		for ancestor in self.ancestors(true) {
			if let Ok(value) = ancestor.try_get_raw(key) {
				return Ok(value);
			}
		}

		Err(ComponentMissingError { key: key.raw() })
	}
}

// === Errors === //

#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
#[error("component {key:?} missing from `Obj`")]
pub struct ComponentMissingError {
	pub key: RawTypedKey,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
#[error("failed to lock component with key {key:?}")]
pub struct ComponentLockError {
	pub error: LockError,
	pub key: RawTypedKey,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Error)]
pub enum BorrowError {
	#[error("failed to find component in `Obj`")]
	ComponentMissing(#[from] ComponentMissingError),
	#[error("failed to borrow component from `Obj`")]
	LockError(#[from] ComponentLockError),
}

// === Multi-borrow === //

pub trait MultiBorrowTarget<'a>: Sized {
	fn try_borrow_from<O: ?Sized + ObjLike>(obj: &'a O) -> Result<Self, BorrowError>;
}

impl<'a, T: ?Sized + 'static> MultiBorrowTarget<'a> for &'a T {
	fn try_borrow_from<O: ?Sized + ObjLike>(obj: &'a O) -> Result<Self, BorrowError> {
		obj.try_get().map_err(From::from)
	}
}

impl<'a, T: ?Sized + 'static> MultiBorrowTarget<'a> for ARef<'a, T> {
	fn try_borrow_from<O: ?Sized + ObjLike>(obj: &'a O) -> Result<Self, BorrowError> {
		obj.try_borrow()
	}
}

impl<'a, T: ?Sized + 'static> MultiBorrowTarget<'a> for AMut<'a, T> {
	fn try_borrow_from<O: ?Sized + ObjLike>(obj: &'a O) -> Result<Self, BorrowError> {
		obj.try_borrow_mut()
	}
}

macro impl_tup_obj_borrowable($($name:ident: $field:tt),*) {
	impl<'a, $($name: MultiBorrowTarget<'a>),*> MultiBorrowTarget<'a> for ($($name,)*) {
		#[allow(unused_variables)]
		fn try_borrow_from<O: ?Sized + ObjLike>(obj: &'a O) -> Result<Self, BorrowError> {
			Ok(($($name::try_borrow_from(obj)?,)*))
		}
	}
}

impl_tuples!(impl_tup_obj_borrowable);

// === Keys === //

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct TypedKey<T: ?Sized> {
	_ty: PhantomData<fn(T) -> T>,
	raw: RawTypedKey,
}

impl<T: ?Sized> TypedKey<T> {
	pub unsafe fn from_raw(raw: RawTypedKey) -> TypedKey<T> {
		Self {
			_ty: PhantomData,
			raw,
		}
	}

	pub fn raw(&self) -> RawTypedKey {
		self.raw
	}
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct RawTypedKey(TypedKeyRawInner);

impl Debug for RawTypedKey {
	#[rustfmt::skip]
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match &self.0 {
			TypedKeyRawInner::Static(key) => {
				f.debug_tuple("RawTypedKey::Static").field(key).finish()
			}
			TypedKeyRawInner::Proxy(key) => {
				f.debug_tuple("RawTypedKey::Proxy").field(key).finish()
			}
			TypedKeyRawInner::Runtime(key) => {
				f.debug_tuple("RawTypedKey::Runtime").field(key).finish()
			}
		}
	}
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
enum TypedKeyRawInner {
	Static(FancyTypeId),
	Proxy(FancyTypeId),
	Runtime(u64),
}

pub fn typed_key<T: ?Sized + 'static>() -> TypedKey<T> {
	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Static(FancyTypeId::of::<T>())),
	}
}

pub fn proxy_key<T: ?Sized + 'static + ProxyKeyType>() -> TypedKey<T::Provides> {
	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Proxy(FancyTypeId::of::<T>())),
	}
}

pub fn dyn_key<T: ?Sized + 'static>() -> TypedKey<T> {
	static GEN: AtomicU64 = AtomicU64::new(0);

	TypedKey {
		_ty: PhantomData,
		raw: RawTypedKey(TypedKeyRawInner::Runtime(
			GEN.try_generate_ref().unwrap_pretty(),
		)),
	}
}

#[doc(hidden)]
pub trait ProxyKeyType {
	type Provides: ?Sized + 'static;
}

pub macro proxy_key($(
	$(#[$macro_meta:meta])*
	$vis:vis proxy $name:ident($target:ty);
)*) {$(
	$(#[$macro_meta])*
	$vis struct $name;

	impl ProxyKeyType for $name {
		type Provides = $target;
	}
)*}

// === Alias lists === //

pub unsafe trait AliasList<T: Sized> {
	unsafe fn push_aliases(self, map: &mut Obj, ptr: NonNull<T>);
}

unsafe impl<T, U> AliasList<T> for TypedKey<U>
where
	T: Sized + Unsize<U>,
	U: ?Sized + 'static,
{
	unsafe fn push_aliases(self, map: &mut Obj, ptr: NonNull<T>) {
		// Unsize the value and convert it back into a pointer
		let ptr = (ptr.as_ref() as &U) as *const U as *mut U;
		let ptr = NonNull::new_unchecked(ptr);

		// Insert the entry
		#[rustfmt::skip]
		map.comps.insert(
			self.raw(),
			ObjEntry::new_alias(ptr, &mut map.bump)
		);
	}
}

macro tup_impl_alias_list($($name:ident: $field:tt),*) {
	unsafe impl<_Src: Sized $(,$name: AliasList<_Src>)*> AliasList<_Src> for ($($name,)*) {
		#[allow(unused_variables)]
		unsafe fn push_aliases(self, obj: &mut Obj, ptr: NonNull<_Src>) {
			$( self.$field.push_aliases(obj, ptr); )*
		}
	}
}

impl_tuples!(tup_impl_alias_list);

// === Event trait creation === //

pub macro event_trait {
	// Muncher base case
	() => {},

	// Immutable
	(
		$(#[$attr:meta])*
		$vis:vis trait
			$name:ident
			$(::<$($generic_param:ident),*$(,)?>)?
			::
			$fn_name:ident
			$(<
				$($lt_decl:lifetime),*
				$(,)?
			>)?
		(
			&self,
			$($arg_name:ident: $arg_ty:ty),*
			$(,)?
		) $(-> $ret:ty)?;

		$($rest:tt)*
	) => {
		$(#[$attr:meta])*
		$vis trait $name $(<$($generic_param),*>)? {
			fn $fn_name $(<$($lt_decl),*>)? (&self, $($arg_name: $arg_ty),*) $(-> $ret)?;
		}

		impl<F $(,$($generic_param),*)?> $name $(<$($generic_param),*>)? for F
		where
			F: $(for<$($lt_decl),*>)? Fn($($arg_ty),*) $(-> $ret)?,
		{
			fn $fn_name $(<$($lt_decl),*>)? (&self, $($arg_name: $arg_ty),*) $(-> $ret)? {
				(self)($($arg_name),*)
			}
		}

		event_trait!($($rest)*);
	},

	// Mutable
	(
		$(#[$attr:meta])*
		$vis:vis trait
			$name:ident
			$(::<$($generic_param:ident),*$(,)?>)?
			::
			$fn_name:ident
			$(<
				$($lt_decl:lifetime),*
				$(,)?
			>)?
		(
			&mut self,
			$($arg_name:ident: $arg_ty:ty),*
			$(,)?
		) $(-> $ret:ty)?;

		$($rest:tt)*
	) => {
		$(#[$attr:meta])*
		$vis trait $name $(<$($generic_param),*>)? {
			fn $fn_name $(<$($lt_decl),*>)? (&mut self, $($arg_name: $arg_ty),*) $(-> $ret)?;
		}

		impl<F $(,$($generic_param),*)?> $name $(<$($generic_param),*>)? for F
		where
			F: $(for<$($lt_decl),*>)? FnMut($($arg_ty),*) $(-> $ret)?,
		{
			fn $fn_name $(<$($lt_decl),*>)? (&mut self, $($arg_name: $arg_ty),*) $(-> $ret)? {
				(self)($($arg_name),*)
			}
		}

		event_trait!($($rest)*);
	},
}
