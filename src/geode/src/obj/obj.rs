impl<T: ?Sized + ObjPointee> From<Obj<T>> for RawObj {
	fn from(obj: Obj<T>) -> Self {
		obj.raw()
	}
}

impl RawObj {
	pub unsafe fn to_typed_unchecked<T: ?Sized + ObjPointee>(
		&self,
		meta: <T as Pointee>::Metadata,
	) -> Obj<T> {
		Obj { raw: *self, meta }
	}
}

// === Obj === //

pub struct Obj<T: ?Sized + ObjPointee> {
	raw: RawObj,
	meta: <T as Pointee>::Metadata,
}

impl<T: ?Sized + ObjPointee + fmt::Debug> fmt::Debug for Obj<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let session = LocalSessionGuard::new();
		let s = session.handle();

		let value = self.try_get(s);

		f.debug_struct("Obj")
			.field("gen", &self.raw.gen.gen())
			.field("value", &value)
			.finish_non_exhaustive()
	}
}

impl<T: ?Sized + ObjPointee> Copy for Obj<T> {}

impl<T: ?Sized + ObjPointee> Clone for Obj<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized + ObjPointee> Eq for Obj<T> {}

impl<T: ?Sized + ObjPointee> PartialEq for Obj<T> {
	fn eq(&self, other: &Self) -> bool {
		self.raw == other.raw
	}
}

impl<T: ?Sized + ObjPointee> hash::Hash for Obj<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.raw.hash(state);
	}
}

impl<T: ?Sized + ObjPointee> Borrow<RawObj> for Obj<T> {
	fn borrow(&self) -> &RawObj {
		&self.raw
	}
}

impl<T: ?Sized + ObjPointee> Destructible for Obj<T> {
	fn destruct(self) {
		LocalSessionGuard::with_new(|session| {
			self.destroy(session.handle());
		})
	}
}

impl<T: Sized + ObjPointee + Sync> Obj<T> {
	#[inline(always)]
	pub fn new(session: Session, value: T) -> Owned<Self> {
		Self::new_in_raw(session, 0xFF, value)
	}
}

impl<T: Sized + ObjPointee> Obj<T> {
	#[inline(always)]
	pub fn new_in(session: Session, lock: Lock, value: T) -> Owned<Self> {
		Self::new_in_raw(session, lock.0, value)
	}

	#[inline(always)]
	fn new_in_raw(session: Session, lock: u8, value: T) -> Owned<Self> {
		// Allocate slot
		let (slot, gen, initial_ptr) =
			db::allocate_new_obj(session, ReflectType::of::<T>(), Layout::new::<T>(), lock);

		// Write initial data
		let initial_ptr = initial_ptr.cast::<T>();

		unsafe {
			initial_ptr.write(value);
		}

		// Obtain pointer metadata (should always be `()` but we do this anyways because `T: Sized`
		// does not imply `<T as Pointee>::Metadata == ()` to the type checker yet)
		let (_, meta) = initial_ptr.to_raw_parts();

		Owned::new(Self {
			raw: RawObj { slot, gen },
			meta,
		})
	}
}

impl<T: ?Sized + ObjPointee> Obj<T> {
	// Fetching
	pub fn try_get<'a>(&self, session: Session<'a>) -> Result<&'a T, ObjGetError> {
		let base_addr = self.raw.try_get_ptr(session)?;
		let ptr = ptr::from_raw_parts(base_addr.as_ptr() as *const (), self.meta);

		Ok(unsafe { &*ptr })
	}

	pub fn get<'a>(&self, session: Session<'a>) -> &'a T {
		let base_addr = self.raw.get_ptr(session);
		let ptr = ptr::from_raw_parts(base_addr.as_ptr() as *const (), self.meta);

		unsafe { &*ptr }
	}

	pub fn weak_get<'a>(&self, session: Session<'a>) -> Result<&'a T, ObjDeadError> {
		ObjGetError::unwrap_weak(self.try_get(session))
	}

	// Casting
	pub fn raw(&self) -> RawObj {
		self.raw
	}

	pub unsafe fn transmute_unchecked_with_meta<U: ?Sized + ObjPointee>(
		&self,
		meta: <U as Pointee>::Metadata,
	) -> Obj<U> {
		// Safety: provided by caller
		self.raw().to_typed_unchecked(meta)
	}

	pub unsafe fn transmute_unchecked<U: ?Sized + ObjPointee>(&self) -> Obj<U> {
		self.transmute_unchecked_with_meta(sizealign_checked_transmute(self.meta))
	}

	pub fn unsize<U>(&self) -> Obj<U>
	where
		T: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		let ptr = ptr::from_raw_parts::<T>(ptr::null(), self.meta);
		let ptr = ptr as *const U;
		let (_, meta) = ptr.to_raw_parts();

		unsafe { self.transmute_unchecked_with_meta(meta) }
	}

	pub fn wrap_transparent<U>(&self) -> Obj<U>
	where
		U: TransparentWrapper<T>,
		U: ?Sized + ObjPointee,
	{
		unsafe {
			// Safety: provided by `TransparentWrapper`
			self.transmute_unchecked()
		}
	}

	pub fn peel_transparent<U>(&self) -> Obj<U>
	where
		T: TransparentWrapper<U>,
		U: ?Sized + ObjPointee,
	{
		unsafe {
			// Safety: provided by `TransparentWrapper`
			self.transmute_unchecked()
		}
	}

	pub fn unsize_delegate_borrow<U>(&self) -> Obj<U>
	where
		DelegateAutoBorrow<T>: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		self.wrap_transparent::<DelegateAutoBorrow<T>>().unsize()
	}

	pub fn unsize_delegate_borrow_mut<U>(&self) -> Obj<U>
	where
		DelegateAutoBorrowMut<T>: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		self.wrap_transparent::<DelegateAutoBorrowMut<T>>().unsize()
	}

	// Lifecycle management
	pub fn is_alive_now(&self, session: Session) -> bool {
		self.raw.is_alive_now(session)
	}

	pub fn ptr_gen(&self) -> u64 {
		self.raw.ptr_gen()
	}

	pub fn destroy(&self, session: Session) -> bool {
		self.raw.destroy(session)
	}
}

// === Owned<RawObj> Forwards === //

impl Owned<RawObj> {
	pub fn try_get_ptr(&self, session: Session) -> Result<NonNull<u8>, ObjGetError> {
		self.weak_copy().try_get_ptr(session)
	}

	pub fn get_ptr(&self, session: Session) -> NonNull<u8> {
		self.weak_copy().get_ptr(session)
	}

	pub fn weak_get_ptr(&self, session: Session) -> Result<NonNull<u8>, ObjDeadError> {
		self.weak_copy().weak_get_ptr(session)
	}

	pub fn is_alive_now(&self, session: Session) -> bool {
		self.weak_copy().is_alive_now(session)
	}

	pub fn ptr_gen(&self) -> u64 {
		self.weak_copy().ptr_gen()
	}

	pub fn destroy(self, session: Session) -> bool {
		self.manually_destruct().destroy(session)
	}
}

impl MaybeOwned<RawObj> {
	pub fn try_get_ptr(&self, session: Session) -> Result<NonNull<u8>, ObjGetError> {
		self.weak_copy().try_get_ptr(session)
	}

	pub fn get_ptr(&self, session: Session) -> NonNull<u8> {
		self.weak_copy().get_ptr(session)
	}

	pub fn weak_get_ptr(&self, session: Session) -> Result<NonNull<u8>, ObjDeadError> {
		self.weak_copy().weak_get_ptr(session)
	}

	pub fn is_alive_now(&self, session: Session) -> bool {
		self.weak_copy().is_alive_now(session)
	}

	pub fn ptr_gen(&self) -> u64 {
		self.weak_copy().ptr_gen()
	}

	pub fn destroy(self, session: Session) -> bool {
		self.manually_destruct().destroy(session)
	}
}

// === Owned<Obj> Forwards === //

impl<T: ?Sized + ObjPointee> Owned<Obj<T>> {
	pub fn try_get<'a>(&self, session: Session<'a>) -> Result<&'a T, ObjGetError> {
		self.weak_copy().try_get(session)
	}

	pub fn get<'a>(&self, session: Session<'a>) -> &'a T {
		self.weak_copy().get(session)
	}

	pub fn weak_get<'a>(&self, session: Session<'a>) -> Result<&'a T, ObjDeadError> {
		self.weak_copy().weak_get(session)
	}

	pub fn raw(self) -> Owned<RawObj> {
		self.map(|obj| obj.raw())
	}

	pub unsafe fn transmute_unchecked_with_meta<U: ?Sized + ObjPointee>(
		self,
		meta: <U as Pointee>::Metadata,
	) -> Owned<Obj<U>> {
		self.map(|obj| obj.transmute_unchecked_with_meta(meta))
	}

	pub unsafe fn transmute_unchecked<U: ?Sized + ObjPointee>(self) -> Owned<Obj<U>> {
		self.map(|obj| obj.transmute_unchecked())
	}

	pub fn unsize<U>(self) -> Owned<Obj<U>>
	where
		T: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.unsize())
	}

	pub fn wrap_transparent<U>(self) -> Owned<Obj<U>>
	where
		U: TransparentWrapper<T>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.wrap_transparent())
	}

	pub fn peel_transparent<U>(self) -> Owned<Obj<U>>
	where
		T: TransparentWrapper<U>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.peel_transparent())
	}

	pub fn unsize_delegate_borrow<U>(self) -> Owned<Obj<U>>
	where
		DelegateAutoBorrow<T>: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.unsize_delegate_borrow())
	}

	pub fn unsize_delegate_borrow_mut<U>(self) -> Owned<Obj<U>>
	where
		DelegateAutoBorrowMut<T>: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.unsize_delegate_borrow_mut())
	}

	pub fn is_alive_now(&self, session: Session) -> bool {
		self.weak_copy().is_alive_now(session)
	}

	pub fn ptr_gen(&self) -> u64 {
		self.weak_copy().ptr_gen()
	}

	pub fn destroy(self, session: Session) -> bool {
		self.manually_destruct().destroy(session)
	}
}

impl<T: ?Sized + ObjPointee> MaybeOwned<Obj<T>> {
	pub fn try_get<'a>(&self, session: Session<'a>) -> Result<&'a T, ObjGetError> {
		self.weak_copy().try_get(session)
	}

	pub fn get<'a>(&self, session: Session<'a>) -> &'a T {
		self.weak_copy().get(session)
	}

	pub fn weak_get<'a>(&self, session: Session<'a>) -> Result<&'a T, ObjDeadError> {
		self.weak_copy().weak_get(session)
	}

	pub fn raw(self) -> MaybeOwned<RawObj> {
		self.map(|obj| obj.raw())
	}

	pub unsafe fn transmute_unchecked_with_meta<U: ?Sized + ObjPointee>(
		self,
		meta: <U as Pointee>::Metadata,
	) -> MaybeOwned<Obj<U>> {
		self.map(|obj| obj.transmute_unchecked_with_meta(meta))
	}

	pub unsafe fn transmute_unchecked<U: ?Sized + ObjPointee>(self) -> MaybeOwned<Obj<U>> {
		self.map(|obj| obj.transmute_unchecked())
	}

	pub fn unsize<U>(self) -> MaybeOwned<Obj<U>>
	where
		T: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.unsize())
	}

	pub fn wrap_transparent<U>(self) -> MaybeOwned<Obj<U>>
	where
		U: TransparentWrapper<T>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.wrap_transparent())
	}

	pub fn peel_transparent<U>(self) -> MaybeOwned<Obj<U>>
	where
		T: TransparentWrapper<U>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.peel_transparent())
	}

	pub fn unsize_delegate_borrow<U>(self) -> MaybeOwned<Obj<U>>
	where
		DelegateAutoBorrow<T>: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.unsize_delegate_borrow())
	}

	pub fn unsize_delegate_borrow_mut<U>(self) -> MaybeOwned<Obj<U>>
	where
		DelegateAutoBorrowMut<T>: Unsize<U>,
		U: ?Sized + ObjPointee,
	{
		self.map(|obj| obj.unsize_delegate_borrow_mut())
	}

	pub fn is_alive_now(&self, session: Session) -> bool {
		self.weak_copy().is_alive_now(session)
	}

	pub fn ptr_gen(&self) -> u64 {
		self.weak_copy().ptr_gen()
	}

	pub fn destroy(self, session: Session) -> bool {
		self.manually_destruct().destroy(session)
	}
}

// === Obj extensions === //

pub type ObjRw<T> = Obj<RefCell<T>>;

impl<T: ObjPointee> ObjRw<T> {
	pub fn new_rw(session: Session, lock: Lock, value: T) -> Owned<Self> {
		Self::new_in(session, lock, RefCell::new(value))
	}
}

impl<T: ?Sized + ObjPointee> ObjRw<T> {
	pub fn borrow<'a>(&self, session: Session<'a>) -> Ref<'a, T> {
		self.get(session).borrow()
	}

	pub fn borrow_mut<'a>(&self, session: Session<'a>) -> RefMut<'a, T> {
		self.get(session).borrow_mut()
	}
}

impl<T: ?Sized + ObjPointee> Owned<ObjRw<T>> {
	pub fn borrow<'a>(&self, session: Session<'a>) -> Ref<'a, T> {
		self.weak_copy().borrow(session)
	}

	pub fn borrow_mut<'a>(&self, session: Session<'a>) -> RefMut<'a, T> {
		self.weak_copy().borrow_mut(session)
	}
}

pub trait ObjCtorExt: Sized + ObjPointee {
	fn box_obj(self, session: Session) -> Owned<Obj<Self>>
	where
		Self: Sync,
	{
		Obj::new(session, self)
	}

	fn box_obj_in(self, session: Session, lock: Lock) -> Owned<Obj<Self>> {
		Obj::new_in(session, lock, self)
	}

	fn box_obj_rw(self, session: Session, lock: Lock) -> Owned<Obj<RefCell<Self>>> {
		Obj::new_rw(session, lock, self)
	}
}

impl<T: Sized + ObjPointee> ObjCtorExt for T {}
