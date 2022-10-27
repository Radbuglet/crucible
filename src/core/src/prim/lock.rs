use std::{
	any::TypeId,
	cell::{Ref, RefCell, RefMut},
	collections::HashMap,
	fmt,
	marker::PhantomData,
	mem,
	num::NonZeroU64,
	ops::{Deref, DerefMut},
	ptr,
	sync::{Mutex, MutexGuard},
};

use derive_where::derive_where;
use owning_ref::OwningRefMut;

use crate::{
	debug::{error::ResultExt, type_id::NamedTypeId},
	lang::{
		macros::impl_tuples,
		marker::{PhantomInvariant, PhantomNoSendOrSync},
		polyfill::OptionPoly,
		std_traits::{BorrowState, MutMarker, Mutability, RefMarker, UnsafeCellLike},
		sync::{AssertSync, SyncUnsafeCell},
	},
	mem::ptr::PointeeCastExt,
};

// === BorrowList === //

#[derive(Debug)]
pub struct DynBorrowListBuilder {
	borrows: HashMap<NamedTypeId, Mutability>,
}

impl DynBorrowListBuilder {
	fn new() -> Self {
		Self {
			borrows: HashMap::default(),
		}
	}

	pub fn push(&mut self, id: NamedTypeId, mutability: Mutability) {
		self.borrows
			.entry(id)
			.and_modify(|e| *e = e.max_privileges(mutability))
			.or_insert(mutability);
	}
}

pub unsafe trait BorrowList {
	fn can_lock_ref<L: ?Sized + 'static>() -> bool;
	fn can_lock_mut<L: ?Sized + 'static>() -> bool;
	fn check_compat(target: &impl Session);
	fn dump_borrows(builder: &mut DynBorrowListBuilder);
}

unsafe impl<T: ?Sized + 'static> BorrowList for RefMarker<T> {
	fn can_lock_ref<L: ?Sized + 'static>() -> bool {
		TypeId::of::<T>() == TypeId::of::<L>()
	}

	fn can_lock_mut<L: ?Sized + 'static>() -> bool {
		false
	}

	fn check_compat(target: &impl Session) {
		assert!(
			target.can_lock_ref::<T>(),
			"expected the ability to lock {target:?} immutably."
		);
	}

	fn dump_borrows(set: &mut DynBorrowListBuilder) {
		set.push(NamedTypeId::of::<T>(), Mutability::Immutable);
	}
}

unsafe impl<T: ?Sized + 'static> BorrowList for MutMarker<T> {
	fn can_lock_ref<L: ?Sized + 'static>() -> bool {
		TypeId::of::<T>() == TypeId::of::<L>()
	}

	fn can_lock_mut<L: ?Sized + 'static>() -> bool {
		TypeId::of::<T>() == TypeId::of::<L>()
	}

	fn check_compat(target: &impl Session) {
		assert!(
			target.can_lock_mut::<T>(),
			"expected the ability to lock {target:?} mutably."
		);
	}

	fn dump_borrows(set: &mut DynBorrowListBuilder) {
		set.push(NamedTypeId::of::<T>(), Mutability::Mutable);
	}
}

macro impl_borrow_list($($para:ident:$field:tt),*) {
	unsafe impl<$($para: BorrowList),*> BorrowList for ($($para,)*) {
		fn can_lock_ref<L: ?Sized + 'static>() -> bool {
			$($para::can_lock_ref::<L>() ||)* false
		}

		fn can_lock_mut<L: ?Sized + 'static>() -> bool {
			$($para::can_lock_mut::<L>() ||)* false
		}

		#[allow(unused)]
		fn check_compat(target: &impl Session) {
			$($para::check_compat(target);)*
		}

		#[allow(unused)]
		fn dump_borrows(set: &mut DynBorrowListBuilder) {
			$($para::dump_borrows(set);)*
		}
	}
}

impl_tuples!(impl_borrow_list);

// === Session === //

type LockDBHandle = OwningRefMut<MutexGuard<'static, Option<LockDB>>, LockDB>;

#[derive(Debug, Default)]
struct LockDB {
	borrows: HashMap<NamedTypeId, BorrowState>,
}

impl LockDB {
	fn get() -> LockDBHandle {
		static INSTANCE: Mutex<Option<LockDB>> = Mutex::new(None);

		OwningRefMut::new(INSTANCE.lock().unwrap_pretty())
			.map_mut(|guard| guard.get_or_insert_with(Default::default))
	}

	fn can_borrow_as(&self, id: NamedTypeId, privileges: Mutability) -> bool {
		self.borrows.get(&id).is_none_or(|borrow| match privileges {
			Mutability::Mutable => false,
			Mutability::Immutable => borrow.mutability() == Mutability::Immutable,
		})
	}

	fn borrow_as(&mut self, id: NamedTypeId, privileges: Mutability) {
		debug_assert!(self.can_borrow_as(id, privileges));

		match privileges {
			Mutability::Immutable => {
				self.borrows
					.entry(id)
					.and_modify(|state| {
						*state = state.as_immutably_reborrowed().unwrap_or_else(|| {
							panic!("Attempted to borrow lock with ID {id:?} too many times!")
						});
					})
					.or_insert_with(|| BorrowState::Immutable(NonZeroU64::new(1).unwrap()));
			}
			Mutability::Mutable => {
				self.borrows.insert(id, BorrowState::Mutable);
			}
		}
	}

	fn unborrow(&mut self, id: NamedTypeId) {
		let borrow = self.borrows.get_mut(&id).unwrap();

		if let Some(decremented) = borrow.as_decremented() {
			*borrow = decremented;
		} else {
			self.borrows.remove(&id);
		}
	}
}

pub unsafe trait Session: Sized + fmt::Debug {
	fn can_lock_mut<L: ?Sized + 'static>(&self) -> bool;

	fn can_lock_ref<L: ?Sized + 'static>(&self) -> bool;

	fn as_static<T: BorrowList>(&self) -> &StaticSession<T> {
		// Ensure that the two sessions are compatible.
		T::check_compat(self);

		// Cast the session!
		unsafe { StaticSession::from_dyn_unchecked(Self::as_dyn(self)) }
	}

	fn as_dyn(&self) -> &DynSession;
}

#[derive_where(Debug)]
#[repr(transparent)]
pub struct StaticSession<T: BorrowList> {
	_ty: PhantomInvariant<T>,
	session: DynSession,
}

impl<T: BorrowList> StaticSession<T> {
	pub fn new() -> Self {
		// Collect a list of borrows.
		let borrows = {
			let mut builder = DynBorrowListBuilder::new();
			T::dump_borrows(&mut builder);
			builder.borrows
		};

		// Check whether the DB can accommodate the borrow.
		let mut db = LockDB::get();

		for (id, privileges) in &borrows {
			if !db.can_borrow_as(*id, *privileges) {
				panic!("Failed to borrow {id:?} {}", privileges.adverb());
			}
		}

		// Commit the borrow.
		for (id, privileges) in &borrows {
			db.borrow_as(*id, *privileges);
		}

		drop(db);

		// Construct the session
		let session = DynSession {
			_no_threading: PhantomData,
			borrows,
		};

		Self {
			_ty: PhantomData,
			session,
		}
	}

	pub unsafe fn from_dyn_unchecked(session: &DynSession) -> &Self {
		session.cast_ref_via_ptr(
			|p| p as *const Self, // repr(transparent)
		)
	}
}

unsafe impl<T: BorrowList> Session for StaticSession<T> {
	fn can_lock_mut<L: ?Sized + 'static>(&self) -> bool {
		T::can_lock_mut::<L>()
			|| self
				.session
				.cold_can_lock_dyn(NamedTypeId::of::<L>(), Mutability::Mutable)
	}

	fn can_lock_ref<L: ?Sized + 'static>(&self) -> bool {
		T::can_lock_ref::<L>()
			|| self
				.session
				.cold_can_lock_dyn(NamedTypeId::of::<L>(), Mutability::Immutable)
	}

	fn as_dyn(&self) -> &DynSession {
		&self.session
	}
}

impl<T: BorrowList> Deref for StaticSession<T> {
	type Target = DynSession;

	fn deref(&self) -> &Self::Target {
		&self.session
	}
}

#[derive(Debug)]
pub struct DynSession {
	_no_threading: PhantomNoSendOrSync,
	borrows: HashMap<NamedTypeId, Mutability>,
}

impl DynSession {
	pub fn can_lock_dyn(&self, lock_ty: NamedTypeId, privileges: Mutability) -> bool {
		self.borrows
			.get(&lock_ty)
			.is_some_and(|provided| provided.can_access_as(privileges))
	}

	#[cold]
	#[inline(never)]
	fn cold_can_lock_dyn(&self, lock_ty: NamedTypeId, privileges: Mutability) -> bool {
		self.can_lock_dyn(lock_ty, privileges)
	}
}

unsafe impl Session for DynSession {
	fn can_lock_mut<L: ?Sized + 'static>(&self) -> bool {
		self.can_lock_dyn(NamedTypeId::of::<L>(), Mutability::Mutable)
	}

	fn can_lock_ref<L: ?Sized + 'static>(&self) -> bool {
		self.can_lock_dyn(NamedTypeId::of::<L>(), Mutability::Immutable)
	}

	fn as_dyn(&self) -> &DynSession {
		self
	}
}

impl Drop for DynSession {
	fn drop(&mut self) {
		let mut db = LockDB::get();

		for id in self.borrows.keys().copied() {
			db.unborrow(id);
		}
	}
}

// === NRefCell === //

pub struct NRefCell<T: ?Sized, L: ?Sized + 'static = T> {
	_lock: PhantomInvariant<L>,
	value: AssertSync<RefCell<T>>,
}

impl<T: ?Sized + fmt::Debug, L: ?Sized + 'static> fmt::Debug for NRefCell<T, L> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("NRefCell")
			.field("_lock", &self._lock)
			.finish_non_exhaustive()
	}
}

impl<T: Default, L: ?Sized + 'static> Default for NRefCell<T, L> {
	fn default() -> Self {
		Self {
			_lock: Default::default(),
			value: Default::default(),
		}
	}
}

unsafe impl<T: ?Sized, L: ?Sized + 'static> UnsafeCellLike for NRefCell<T, L> {
	type Inner = T;

	fn get_ptr(&self) -> *mut Self::Inner {
		unsafe { self.value.get() }.as_ptr()
	}

	fn into_inner(self) -> Self::Inner
	where
		Self::Inner: Sized,
	{
		self.value.into_inner().into_inner()
	}
}

impl<T: ?Sized, L: ?Sized + 'static> NRefCell<T, L> {
	pub const fn new(value: T) -> Self
	where
		T: Sized,
	{
		Self {
			_lock: PhantomData,
			value: AssertSync::new(RefCell::new(value)),
		}
	}

	pub fn borrow<'a>(&'a self, s: &'a impl Session) -> NRef<'a, T> {
		assert!(
			s.can_lock_ref::<L>(),
			"{s:?} cannot lock NRefCell protected with lock {:?}",
			NamedTypeId::of::<L>()
		);

		// FIXME: This will hit the cold path if a `StaticSession` only borrows something immutably.
		if s.can_lock_mut::<L>() {
			let borrow = unsafe { self.value.get() }.borrow();

			NRef(NRefInner::Smart(borrow))
		} else {
			let borrow = unsafe { self.get_ref_unchecked() };

			NRef(NRefInner::Dumb(borrow))
		}
	}

	pub fn borrow_mut<'a>(&'a self, s: &'a impl Session) -> NMut<'a, T> {
		assert!(
			s.can_lock_mut::<L>(),
			"{s:?} cannot lock NRefCell protected with lock {:?}",
			NamedTypeId::of::<L>()
		);

		let borrow = unsafe { self.value.get() }.borrow_mut();

		NMut(borrow)
	}

	pub fn can_access_ref(&self, s: &impl Session) -> bool {
		s.can_lock_ref::<L>()
	}

	pub fn can_access_mut(&self, s: &impl Session) -> bool {
		s.can_lock_mut::<L>()
	}

	pub fn replace(&mut self, s: &impl Session, value: T) -> T
	where
		T: Sized,
	{
		mem::replace(&mut self.borrow_mut(s), value)
	}

	pub fn replace_with<F>(&mut self, s: &impl Session, f: F) -> T
	where
		F: FnOnce(&mut T) -> T,
		T: Sized,
	{
		let mut borrow = self.borrow_mut(s);
		let value = f(&mut *borrow);
		mem::replace(&mut borrow, value)
	}
}

// === NRef === //

#[derive(Debug)]
pub struct NRef<'a, T: ?Sized>(NRefInner<'a, T>);

impl<'a, T: ?Sized> NRef<'a, T> {
	pub fn clone(orig: &Self) -> Self {
		let inner = match &orig.0 {
			NRefInner::Dumb(v) => NRefInner::Dumb(*v),
			NRefInner::Smart(v) => NRefInner::Smart(Ref::clone(v)),
		};

		Self(inner)
	}

	pub fn map<U: ?Sized, F>(orig: Self, f: F) -> NRef<'a, U>
	where
		F: FnOnce(&T) -> &U,
	{
		let inner = match orig.0 {
			NRefInner::Dumb(v) => NRefInner::Dumb(f(v)),
			NRefInner::Smart(v) => NRefInner::Smart(Ref::map(v, f)),
		};

		NRef(inner)
	}

	pub fn filter_map<U: ?Sized, F>(orig: Self, f: F) -> Result<NRef<'a, U>, Self>
	where
		F: FnOnce(&T) -> Option<&U>,
	{
		let inner = match orig.0 {
			NRefInner::Dumb(v) => match f(v) {
				Some(val) => Ok(NRefInner::Dumb(val)),
				None => Err(NRefInner::Dumb(v)),
			},
			NRefInner::Smart(v) => match Ref::filter_map(v, f) {
				Ok(v) => Ok(NRefInner::Smart(v)),
				Err(v) => Err(NRefInner::Smart(v)),
			},
		};

		match inner {
			Ok(v) => Ok(NRef(v)),
			Err(v) => Err(NRef(v)),
		}
	}

	pub fn map_split<U: ?Sized, V: ?Sized, F>(orig: Self, f: F) -> (NRef<'a, U>, NRef<'a, V>)
	where
		F: FnOnce(&T) -> (&U, &V),
	{
		let (inner_a, inner_b) = match orig.0 {
			NRefInner::Dumb(v) => {
				let (a, b) = f(v);

				(NRefInner::Dumb(a), NRefInner::Dumb(b))
			}
			NRefInner::Smart(v) => {
				let (a, b) = Ref::map_split(v, f);

				(NRefInner::Smart(a), NRefInner::Smart(b))
			}
		};

		(NRef(inner_a), NRef(inner_b))
	}
}

impl<'a, T: ?Sized> Deref for NRef<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		match &self.0 {
			NRefInner::Dumb(v) => v,
			NRefInner::Smart(v) => v,
		}
	}
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for NRef<'a, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		(&**self).fmt(f)
	}
}

#[derive(Debug)]
enum NRefInner<'a, T: ?Sized> {
	Dumb(&'a T),
	Smart(Ref<'a, T>),
}

// === NMut === //

#[derive(Debug)]
pub struct NMut<'a, T: ?Sized>(RefMut<'a, T>);

impl<'a, T: ?Sized> NMut<'a, T> {
	pub fn map<U: ?Sized, F>(orig: Self, f: F) -> NMut<'a, U>
	where
		F: FnOnce(&mut T) -> &mut U,
	{
		NMut(RefMut::map(orig.0, f))
	}

	pub fn filter_map<U: ?Sized, F>(orig: Self, f: F) -> Result<NMut<'a, U>, Self>
	where
		F: FnOnce(&mut T) -> Option<&mut U>,
	{
		match RefMut::filter_map(orig.0, f) {
			Ok(v) => Ok(NMut(v)),
			Err(v) => Err(NMut(v)),
		}
	}

	pub fn map_split<U: ?Sized, V: ?Sized, F>(orig: Self, f: F) -> (NMut<'a, U>, NMut<'a, V>)
	where
		F: FnOnce(&mut T) -> (&mut U, &mut V),
	{
		let (a, b) = RefMut::map_split(orig.0, f);

		(NMut(a), NMut(b))
	}
}

impl<'a, T: ?Sized> Deref for NMut<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<'a, T: ?Sized> DerefMut for NMut<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for NMut<'a, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		(&**self).fmt(f)
	}
}

// === NCell === //

pub struct NCell<T, L: ?Sized + 'static> {
	_ty: PhantomInvariant<L>,
	value: SyncUnsafeCell<T>,
}

impl<T, L: ?Sized + 'static> fmt::Debug for NCell<T, L> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("NCell")
			.field("_ty", &self._ty)
			.finish_non_exhaustive()
	}
}

impl<T: Default, L: ?Sized + 'static> Default for NCell<T, L> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T, L: ?Sized + 'static> From<T> for NCell<T, L> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

impl<T, L: ?Sized + 'static> NCell<T, L> {
	pub const fn new(value: T) -> Self {
		Self {
			_ty: PhantomData,
			value: SyncUnsafeCell::new(value),
		}
	}

	pub fn set(&self, s: &impl Session, value: T) {
		assert!(
			s.can_lock_mut::<L>(),
			"{s:?} cannot write to NCell protected with lock {:?}",
			NamedTypeId::of::<L>()
		);

		unsafe {
			*self.value.get_mut_unchecked() = value;
		}
	}

	pub fn swap<L2: ?Sized + 'static>(&self, s: &impl Session, other: &NCell<T, L2>) {
		assert!(
			s.can_lock_mut::<L>(),
			"{s:?} cannot write to NCell protected with lock {:?}",
			NamedTypeId::of::<L>()
		);
		assert!(
			s.can_lock_mut::<L2>(),
			"{s:?} cannot write to NCell protected with lock {:?}",
			NamedTypeId::of::<L2>()
		);

		if ptr::eq(self.get_ptr(), other.get_ptr()) {
			return;
		}

		unsafe { ptr::swap(self.get_ptr(), other.get_ptr()) }
	}

	pub fn replace(&self, s: &impl Session, value: T) -> T {
		assert!(
			s.can_lock_mut::<L>(),
			"{s:?} cannot write to NCell protected with lock {:?}",
			NamedTypeId::of::<L>()
		);

		unsafe { mem::replace(self.get_mut_unchecked(), value) }
	}

	pub fn can_access_ref(&self, s: &impl Session) -> bool {
		s.can_lock_ref::<L>()
	}

	pub fn can_access_mut(&self, s: &impl Session) -> bool {
		s.can_lock_mut::<L>()
	}
}

impl<T: Copy, L: ?Sized + 'static> NCell<T, L> {
	pub fn get(&self, s: &impl Session) -> T {
		assert!(
			s.can_lock_ref::<L>(),
			"{s:?} cannot read to NCell protected with lock {:?}",
			NamedTypeId::of::<L>()
		);

		unsafe { *self.get_ptr() }
	}

	pub fn update<F: FnOnce(T) -> T>(&self, s: &impl Session, f: F) -> T {
		assert!(
			s.can_lock_mut::<L>(),
			"{s:?} cannot write to NCell protected with lock {:?}",
			NamedTypeId::of::<L>()
		);

		unsafe {
			let p = self.get_ptr();
			let old = *p;
			let new = f(old);
			*p = new;
			new
		}
	}
}

impl<T: Default, L: ?Sized + 'static> NCell<T, L> {
	pub fn take(&self, s: &impl Session) -> T {
		self.replace(s, T::default())
	}
}

unsafe impl<T, L: ?Sized + 'static> UnsafeCellLike for NCell<T, L> {
	type Inner = T;

	fn get_ptr(&self) -> *mut Self::Inner {
		self.value.get_ptr()
	}

	fn into_inner(self) -> Self::Inner
	where
		Self::Inner: Sized,
	{
		self.value.into_inner()
	}
}

// === Tests === //

#[cfg(test)]
mod tests {
	use super::*;

	struct MyLock1;
	struct MyLock2;

	#[test]
	fn lock_test() {
		let cell_1 = NRefCell::<_, MyLock1>::new(3);
		let cell_2 = NRefCell::<_, MyLock2>::new(3);
		let cell_3 = NCell::<u32, MyLock1>::new(32);

		{
			let s = StaticSession::<(MutMarker<MyLock1>, RefMarker<MyLock2>)>::new();

			cell_1.borrow(&s);
			cell_1.borrow_mut(&s);

			cell_2.borrow(&s);
			assert!(!cell_2.can_access_mut(&s));

			let s2 = StaticSession::<RefMarker<MyLock2>>::new();
			assert!(!cell_1.can_access_ref(&s2));
			assert!(!cell_1.can_access_mut(&s2));

			cell_2.borrow(&s2);
			assert!(!cell_2.can_access_mut(&s2));

			let s3 = &*s2;
			assert!(!cell_1.can_access_ref(s3));
			assert!(!cell_1.can_access_mut(s3));

			cell_2.borrow(s3);
			assert!(!cell_2.can_access_mut(s3));
		}
		{
			let s = StaticSession::<(MutMarker<MyLock1>, MutMarker<MyLock2>)>::new();

			cell_1.borrow(&s);
			cell_1.borrow_mut(&s);

			cell_2.borrow(&s);
			cell_2.borrow_mut(&s);

			let s2: &DynSession = &*s;

			cell_1.borrow(s2);
			cell_1.borrow_mut(s2);

			cell_2.borrow(s2);
			cell_2.borrow_mut(s2);

			let s3 = s2.as_static::<RefMarker<MyLock1>>();

			cell_1.borrow(s3);
			cell_1.borrow_mut(s3);

			cell_2.borrow(s3);
			cell_2.borrow_mut(s3);

			cell_3.set(s3, 3);
			cell_3.update(s3, |x| x + 1);
			assert_eq!(cell_3.get(s3), 4);
		}
	}

	#[test]
	fn mutability_matrix() {
		assert!(Mutability::Mutable.can_access_as(Mutability::Immutable));
		assert!(Mutability::Mutable.can_access_as(Mutability::Mutable));

		assert!(Mutability::Immutable.can_access_as(Mutability::Immutable));
		assert!(!Mutability::Immutable.can_access_as(Mutability::Mutable));
	}
}
