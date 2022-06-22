use crate::util::{
	cell::{lot_new_mutex, MutexedUnsafeCell},
	marker::PhantomNoSendOrSync,
	number::U8Alloc,
};
use arr_macro::arr;
use parking_lot::Mutex;
use std::{cell::Cell, marker::PhantomData};

#[derive(Default)]
struct SessionDB {
	session_ids: U8Alloc,
}

static SESSION_DB: Mutex<SessionDB> = lot_new_mutex(SessionDB {
	session_ids: U8Alloc::new(),
});

thread_local! {
	static LOCAL_SESSION_ID: Cell<Option<SessionId>> = Default::default();
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct SessionId(u8);

impl SessionId {
	pub fn current() -> Option<Self> {
		LOCAL_SESSION_ID.with(|cell| cell.get())
	}

	pub fn slot(self) -> u8 {
		self.0
	}
}

pub struct Session<'d> {
	_ty: PhantomData<&'d mut ()>,
	_no_send_or_sync: PhantomNoSendOrSync,
	id: SessionId,
}

impl Session<'_> {
	pub(crate) fn new_raw() -> Self {
		assert_eq!(
			SessionId::current(),
			None,
			"Cannot create more than one `Session` on a given thread."
		);

		let id = SESSION_DB.lock().session_ids.alloc();
		assert_ne!(
			id, 0xFF,
			"Cannot create more than `255` concurrent sessions."
		);

		let id = SessionId(id);
		LOCAL_SESSION_ID.with(|thread_id| thread_id.set(Some(id)));
		Self {
			_ty: PhantomData,
			_no_send_or_sync: PhantomData,
			id,
		}
	}

	pub fn id(&self) -> SessionId {
		self.id
	}
}

impl Drop for Session<'_> {
	fn drop(&mut self) {
		SESSION_DB.lock().session_ids.free(self.id.slot());
		LOCAL_SESSION_ID.with(|thread_id| thread_id.set(None));
	}
}

pub struct SessionStorage<T> {
	slots: [MutexedUnsafeCell<Option<T>>; 256],
}

impl<T> SessionStorage<T> {
	pub const fn new() -> Self {
		Self {
			slots: arr![MutexedUnsafeCell::new(None); 256],
		}
	}

	pub fn get<'a>(&'a self, session: &'a Session) -> Option<&'a T> {
		unsafe {
			// Safety: we know, by the fact that `session` cannot be shared between threads, that
			// we are on the only thread with access to this value.
			self.slots[session.id().slot() as usize]
				.get_ref_unchecked()
				.as_ref()
		}
	}

	pub fn get_mut<'a>(&'a self, session: &'a mut Session) -> &'a mut Option<T> {
		unsafe {
			// Safety: see `.get()`
			self.slots[session.id().slot() as usize].get_mut_unchecked()
		}
	}

	pub fn get_or_init_using<'a, F>(&'a self, session: &'a Session, mut initializer: F) -> &'a T
	where
		F: FnMut() -> T,
	{
		// Try to acquire via existing reference
		if let Some(data) = self.get(session) {
			return data;
		}

		// Initialize a value
		let value = initializer();

		// Make sure that someone hasn't initialized (and potentially obtained a reference to) the
		// cell before we did.
		assert!(self.get(session).is_none());

		// Initialize and return
		unsafe {
			// Safety: we know that no references to the `Option` because it is still `None` (we only
			// return references to the inner value of the `Option` and `get_mut` would require a
			// mutable reference to the session we're already borrowing immutably). We also know,
			// for the same reasoning as above, that our thread has exclusive ownership of this cell.
			// Thus, this is safe.
			let slot = self.slots[session.id().slot() as usize].get_mut_unchecked();

			// This cannot run a destructor to observe the mutable borrow because we already checked
			// that it was none.
			*slot = Some(value);

			// Safety: we just need to make sure to return an immutable reference now.
			slot.as_ref().unwrap()
		}
	}

	pub fn get_mut_or_init_using<'a, F>(
		&'a self,
		session: &'a mut Session,
		mut initializer: F,
	) -> &'a mut T
	where
		F: FnMut() -> T,
	{
		match self.get_mut(session) {
			Some(session) => session,
			outer @ None => {
				*outer = Some(initializer());
				outer.as_mut().unwrap()
			}
		}
	}
}

impl<T: Default> SessionStorage<T> {
	pub fn get_or_init<'a>(&'a self, session: &'a Session) -> &'a T {
		self.get_or_init_using(session, Default::default)
	}

	pub fn get_mut_or_init<'a>(&'a self, session: &'a mut Session) -> &'a mut T {
		self.get_mut_or_init_using(session, Default::default)
	}
}

// TODO: Add support for auto-init and auto-drop `SessionStorages` to make proving the validity of
//  certain operations easier and to move initialization routines out of the fast path.
