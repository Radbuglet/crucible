use crucible_core::{
	const_hacks::ConstSafeMutRef,
	drop_guard::{DropGuard, DropGuardHandler},
};

use std::{
	cell::{Cell, RefCell},
	fmt, hash, mem, ptr,
};

use super::debug::{DebugLabel, SerializedDebugLabel};

// === SessionManager === //

mod db {
	use std::{marker::PhantomData, mem};

	use crucible_core::{
		const_hacks::ConstSafeMutRef, drop_guard::DropGuard, marker::PhantomNoSync,
	};
	use parking_lot::Mutex;

	use crate::util::threading::new_lot_mutex;

	pub trait SessionStateContainer: 'static + Default + Send {
		unsafe fn init(&mut self);
		unsafe fn deinit(&mut self);
	}

	pub struct SessionStatePointee<C> {
		_no_sync: PhantomNoSync,
		pub index: usize,
		pub state_container: C,
	}

	pub struct SessionManager<C: 'static>(Mutex<SessionManagerInner<C>>);

	struct SessionManagerInner<C: 'static> {
		free: Vec<ConstSafeMutRef<'static, SessionStatePointee<C>>>,
		total_session_count: usize,
	}

	impl<C> SessionManager<C> {
		pub const fn new() -> Self {
			Self(new_lot_mutex(SessionManagerInner {
				free: Vec::new(),
				total_session_count: 0,
			}))
		}
	}

	impl<C: SessionStateContainer> SessionManager<C> {
		pub fn allocate_session(&self) -> &'static mut SessionStatePointee<C> {
			// Reserve a session
			let session = {
				let mut inner = self.0.lock();
				if let Some(free) = inner.free.pop() {
					free.into_ref()
				} else {
					// Increment index counter
					let index = inner.total_session_count;
					inner.total_session_count.checked_add(1).expect(
						"Allocated more than `usize::MAX` different sessions. \
		 				 Given that sessions are reused, this overflow was likely caused by a memory leak.",
					);

					// Create session
					drop(inner);
					let session = Box::leak(Box::new(SessionStatePointee {
						_no_sync: PhantomData,
						index,
						state_container: C::default(),
					}));
					session
				}
			};

			// Initialize the container
			let mut session = DropGuard::new(session, |session| {
				self.0.lock().free.push(ConstSafeMutRef::new(session));
			});

			unsafe {
				SessionStateContainer::init(&mut session.state_container);
			}

			DropGuard::defuse(session)
		}

		pub fn deallocate_session(&self, session: &'static mut SessionStatePointee<C>) {
			let mut session = DropGuard::new(session, |session| {
				self.0.lock().free.push(ConstSafeMutRef::from(session));
			});

			unsafe {
				SessionStateContainer::deinit(&mut session.state_container);
			}

			drop(session);
		}

		pub fn acquire_free_sessions(
			&self,
		) -> (Vec<ConstSafeMutRef<'static, SessionStatePointee<C>>>, usize) {
			// Acquire session list
			let (free_sessions, total_session_count) = {
				let mut inner = self.0.lock();

				(
					mem::replace(&mut inner.free, Vec::new()),
					inner.total_session_count,
				)
			};

			// Initialize all the sessions
			let mut free_sessions = DropGuard::new(free_sessions, |free_sessions| {
				self.0.lock().free.extend(free_sessions);
			});

			for session in free_sessions.iter_mut() {
				unsafe {
					SessionStateContainer::init(&mut session.as_ref().state_container);
				}
			}

			// Return the group if they were all successfully initialized. Otherwise, place them
			// back in the free list.
			(DropGuard::defuse(free_sessions), total_session_count)
		}

		pub fn release_free_sessions(
			&self,
			list: Vec<ConstSafeMutRef<'static, SessionStatePointee<C>>>,
		) {
			let mut list = DropGuard::new(list, |list| {
				self.0.lock().free.extend(list);
			});

			for session in list.iter_mut() {
				unsafe {
					SessionStateContainer::deinit(&mut session.as_ref().state_container);
				}
			}

			drop(list);
		}

		pub fn total_session_count(&self) -> usize {
			self.0.lock().total_session_count
		}
	}
}

// === Static storage === //

pub(crate) trait StaticStorageHandler {
	type Comp: 'static + Send;

	fn init_comp(comp: &mut Option<Self::Comp>);
	fn deinit_comp(comp: &mut Option<Self::Comp>) {
		let _ = comp;
	}
}

pub(crate) unsafe trait StaticStorageGetter: StaticStorageHandler {
	fn get(session: Session) -> &Self::Comp;
}

macro static_storage_container(
	$vis:vis struct $name:ident {
		$($field:ident: $comp:ty),*
		$(,)?
	}
) {
	$vis struct $name {
		$($field: Option<<$comp as StaticStorageHandler>::Comp>),*
	}

	impl Default for $name {
		fn default() -> Self {
			let mut container = Self {
				$($field: None),*
			};

			unsafe {
				db::SessionStateContainer::init(&mut container);
			}

			container
		}
	}

	impl db::SessionStateContainer for $name {
		unsafe fn init(&mut self) {
			$({
				let comp = &mut self.$field;
				<$comp as StaticStorageHandler>::init_comp(comp);
				assert!(
					comp.is_some(),
					"`StaticStorageHandler` initializer for {} failed to set component slot to an initial value.",
					std::any::type_name::<$comp>(),
				);
			})*
		}

		unsafe fn deinit(&mut self) {
			$(<$comp as StaticStorageHandler>::deinit_comp(&mut self.$field);)*
		}
	}

	$(
		unsafe impl StaticStorageGetter for $comp {
			fn get(session: Session) -> &Self::Comp {
				unsafe {
					let container = &session.state.state_container;
					container.$field.as_ref().unwrap_unchecked()
				}
			}
		}
	)*
}

// === Global state === //

type SessionStatePointee = db::SessionStatePointee<SessionStateContainer>;

static DB: db::SessionManager<SessionStateContainer> = db::SessionManager::new();

static_storage_container! {
	struct SessionStateContainer {
		debug_name: SessionStateDebugName,
		lock_manager: super::lock::SessionStateLockManager,
		slot_manager: super::object_db::SessionStateSlotManager,
		gc_manager: super::gc::SessionStateGcManager,
	}
}

thread_local! {
	static LOCAL_SESSION_GUARD_STATE: (Cell<usize>, Cell<*mut SessionStatePointee>) = (
		Cell::new(0),
		Cell::new(ptr::null_mut()),
	);
}

// === MovableSessionGuard === //

pub struct MovableSessionGuard {
	// Mutable references to `SessionStatePointee` are `Send` but not `Sync` so this type exhibits
	// the proper threading semantics.
	state: DropGuard<&'static mut SessionStatePointee, MovableSessionGuardDctor>,
}

struct MovableSessionGuardDctor;

impl DropGuardHandler<&'static mut SessionStatePointee> for MovableSessionGuardDctor {
	fn destruct(self, value: &'static mut SessionStatePointee) {
		DB.deallocate_session(value);
	}
}

impl MovableSessionGuard {
	pub fn new() -> Self {
		Self {
			state: DropGuard::new(DB.allocate_session(), MovableSessionGuardDctor),
		}
	}

	pub fn acquire_free() -> (FreeSessionIter, usize) {
		let (free_vec, total_session_count) = DB.acquire_free_sessions();
		(
			FreeSessionIter(DropGuard::new(free_vec, FreeSessionIterDctor)),
			total_session_count,
		)
	}

	pub fn total_session_count() -> usize {
		DB.total_session_count()
	}

	pub fn handle(&self) -> Session {
		Session {
			state: &**self.state,
		}
	}

	pub fn make_local(self) -> LocalSessionGuard {
		LOCAL_SESSION_GUARD_STATE.with(|(rc, thread_state)| {
			assert_eq!(
				rc.get(), 0,
				"Attempted to make a `MovableSessionGuard` local while another local session was extant."
			);

			// Update state
			let state = DropGuard::defuse(self.state) as *mut _;
			thread_state.set(state);
			rc.set(1);

			// Construct guard
			LocalSessionGuard { state }
		})
	}
}

type FreeSessionVec = Vec<ConstSafeMutRef<'static, SessionStatePointee>>;

pub struct FreeSessionIter(DropGuard<FreeSessionVec, FreeSessionIterDctor>);

impl ExactSizeIterator for FreeSessionIter {}

impl Iterator for FreeSessionIter {
	type Item = MovableSessionGuard;

	fn next(&mut self) -> Option<Self::Item> {
		let elem = self.0.pop()?;
		Some(MovableSessionGuard {
			state: DropGuard::new(elem.into_ref(), MovableSessionGuardDctor),
		})
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.0.iter().size_hint()
	}
}

struct FreeSessionIterDctor;

impl DropGuardHandler<FreeSessionVec> for FreeSessionIterDctor {
	fn destruct(self, list: FreeSessionVec) {
		DB.release_free_sessions(list);
	}
}

// === LocalSessionGuard === //

pub struct LocalSessionGuard {
	// Raw pointers are neither `Send` nor `Sync` so this type exhibits the proper threading semantics.
	state: *const SessionStatePointee,
}

impl Clone for LocalSessionGuard {
	fn clone(&self) -> Self {
		Self::new()
	}
}

impl LocalSessionGuard {
	pub fn new() -> Self {
		LOCAL_SESSION_GUARD_STATE.with(|(rc, thread_state)| {
			let mut rc_val = rc.get();
			if rc_val == 0 {
				MovableSessionGuard::new().make_local()
			} else {
				rc_val = rc_val.checked_add(1).expect(
					"Cannot borrow the same `LocalSessionGuard` more than `usize::MAX` times",
				);
				rc.set(rc_val);

				LocalSessionGuard {
					state: thread_state.get(),
				}
			}
		})
	}

	pub fn make_movable(self) -> MovableSessionGuard {
		LOCAL_SESSION_GUARD_STATE.with(|(rc, thread_state)| {
			assert_eq!(rc.get(), 1, "`make_movable` requires that exactly one `LocalSessionGuard` exists on this thread.");
			mem::forget(self);
			rc.set(0);

			MovableSessionGuard {
				state: DropGuard::new(
					unsafe { &mut *thread_state.get() },
					MovableSessionGuardDctor,
				),
			}
		})
	}

	pub fn handle(&self) -> Session {
		Session {
			state: unsafe { &*self.state },
		}
	}
}

impl Drop for LocalSessionGuard {
	fn drop(&mut self) {
		LOCAL_SESSION_GUARD_STATE.with(|(rc, thread_state)| {
			// Update RC
			let new_rc = rc.get();
			debug_assert_ne!(new_rc, 0);
			let new_rc = new_rc - 1;
			rc.set(new_rc);

			// Destroy session if the RC went to zero
			if new_rc == 0 {
				let thread_state = unsafe {
					// Safety: a borrowed form of this raw pointer is only accessible in `Sessions`
					// and `Sessions` can only live for as long the last `LocalSessionGuard`.
					&mut *thread_state.get()
				};

				DB.deallocate_session(thread_state);
			}
		});
	}
}

// === Session === //

#[derive(Copy, Clone)]
pub struct Session<'a> {
	// Immutable references to `SessionStatePointee` are neither `Send` nor `Sync` so this type
	// exhibits the proper threading semantics.
	state: &'a SessionStatePointee,
}

impl hash::Hash for Session<'_> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		(self.state as *const SessionStatePointee).hash(state);
	}
}

impl Eq for Session<'_> {}

impl PartialEq for Session<'_> {
	fn eq(&self, other: &Self) -> bool {
		(self.state as *const SessionStatePointee) == (other.state as *const SessionStatePointee)
	}
}

// === Debug names === //

struct SessionStateDebugName;

impl StaticStorageHandler for SessionStateDebugName {
	type Comp = RefCell<SerializedDebugLabel>;

	fn init_comp(comp: &mut Option<Self::Comp>) {
		*comp = Some(Default::default());
	}

	fn deinit_comp(comp: &mut Option<Self::Comp>) {
		*comp = None;
	}
}

fn fmt_session(f: &mut fmt::Formatter<'_>, struct_name: &str, session: Session<'_>) -> fmt::Result {
	let debug_name = SessionStateDebugName::get(session).borrow();

	f.debug_struct(struct_name)
		.field("debug_name", &debug_name)
		.field("state", &(session.state as *const SessionStatePointee))
		.finish()
}

impl fmt::Debug for MovableSessionGuard {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt_session(f, "MovableSessionGuard", self.handle())
	}
}

impl fmt::Debug for LocalSessionGuard {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt_session(f, "LocalSessionGuard", self.handle())
	}
}

impl fmt::Debug for Session<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt_session(f, "Session", *self)
	}
}

impl Session<'_> {
	pub fn set_debug_name<L: DebugLabel>(self, label: L) {
		// N.B. we serialize the label before locking `SessionDebugNameState`.
		let label = label.to_debug_label();

		*SessionStateDebugName::get(self).borrow_mut() = label;
	}
}
