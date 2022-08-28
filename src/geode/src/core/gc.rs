use std::sync::{Arc, Weak};

use super::{
	lock::{BorrowMutability, UserLock},
	session::{MovableSessionGuard, Session, StaticStorageGetter, StaticStorageHandler},
};

// === Internals === //

mod internal {
	use std::{
		any,
		cell::UnsafeCell,
		mem::{self, ManuallyDrop},
		panic::{catch_unwind, AssertUnwindSafe},
	};

	use bumpalo::Bump;
	use crucible_core::cell::UnsafeCellExt;

	use crate::core::session::Session;

	#[derive(Default)]
	pub struct Executor {
		bump: UnsafeCell<Bump>,
	}

	#[repr(C)]
	struct Entry<T> {
		header: FinalizerHeader,
		value: ManuallyDrop<T>,
	}

	struct FinalizerHeader {
		handler: unsafe fn(Session, *mut Self) -> *mut Self,
	}

	impl Executor {
		pub fn push<H: GcHookOnce>(&self, entry: H) {
			unsafe fn handler<H: GcHookOnce>(
				session: Session,
				base: *mut FinalizerHeader,
			) -> *mut FinalizerHeader {
				let base = base.cast::<Entry<H>>();
				let entry = ManuallyDrop::take(&mut (*base).value);

				let _ = catch_unwind(AssertUnwindSafe(move || {
					entry.process(session);
				}));

				// N.B. this does not run into provenance issues because the pointer keeps the
				// provenance of the allocation chunk, not the provenance of each individual hook.
				//
				// Oh yeah, we also disabled padding by making every hook have the same alignment.
				base.add(1).cast::<FinalizerHeader>()
			}

			assert_eq!(
				mem::align_of::<H>(),
				mem::align_of::<usize>(),
				"{} must have the same alignment as a `usize`.",
				any::type_name::<H>(),
			);

			let bump = unsafe { self.bump.get_mut_unchecked() };
			bump.alloc(Entry {
				header: FinalizerHeader {
					handler: handler::<H>,
				},
				value: ManuallyDrop::new(entry),
			});
		}

		pub unsafe fn process_once(&self, session: Session) {
			let bump = mem::replace(self.bump.get_mut_unchecked(), Bump::new());

			for (start, len) in bump.iter_allocated_chunks_raw() {
				let mut finger = start.cast::<FinalizerHeader>();
				let exclusive_end = start.add(len).cast::<FinalizerHeader>();

				while finger < exclusive_end {
					finger = ((*finger).handler)(session, finger);
				}
			}
		}
	}

	pub trait GcHookOnce: 'static + Sized + Send {
		unsafe fn process(self, session: Session);
	}

	impl<F> GcHookOnce for F
	where
		F: 'static + Sized + Send + FnOnce(Session),
	{
		unsafe fn process(self, session: Session) {
			(self)(session)
		}
	}

	pub trait GcHookMany: 'static + Sized + Send {
		unsafe fn process(&mut self, session: Session);
	}

	impl<F> GcHookMany for F
	where
		F: 'static + Sized + Send + FnMut(Session),
	{
		unsafe fn process(&mut self, session: Session) {
			(self)(session)
		}
	}
}

// === Global state === //

#[derive(Default)]
pub(crate) struct SessionStateGcManager {
	finalizers: internal::Executor,
	compactors: internal::Executor,
}

impl StaticStorageHandler for SessionStateGcManager {
	type Comp = Self;

	fn init_comp(comp: &mut Option<Self::Comp>) {
		if comp.is_none() {
			*comp = Some(Default::default());
		}
	}
}

// === Interface === //

pub use internal::{GcHookMany, GcHookOnce};

impl Session<'_> {
	pub unsafe fn add_gc_finalizer<H: GcHookOnce>(self, hook: H) {
		SessionStateGcManager::get(self).finalizers.push(hook);
	}

	pub unsafe fn add_gc_compactor<H: GcHookOnce>(self, hook: H) {
		SessionStateGcManager::get(self).compactors.push(hook);
	}
}

#[derive(Debug, Clone)]
pub struct PersistentGcHook(Arc<()>);

struct RepeatedGcHook<H, const IS_FINALIZER: bool>(H, Weak<()>);

impl<H: GcHookMany, const IS_FINALIZER: bool> GcHookOnce for RepeatedGcHook<H, IS_FINALIZER> {
	unsafe fn process(mut self, session: Session) {
		if self.1.upgrade().is_some() {
			self.0.process(session);

			if IS_FINALIZER {
				session.add_gc_finalizer(self);
			} else {
				session.add_gc_compactor(self);
			}
		} else {
			drop(self);
		}
	}
}

// TODO: Add a dedicated system to handle these?
impl PersistentGcHook {
	pub unsafe fn new_finalizer<H: GcHookMany>(session: Session, hook: H) {
		let keep_alive = Arc::new(());
		session.add_gc_finalizer(RepeatedGcHook::<H, true>(hook, Arc::downgrade(&keep_alive)));
	}

	pub unsafe fn new_compactor<H: GcHookMany>(session: Session, hook: H) {
		let keep_alive = Arc::new(());
		session.add_gc_finalizer(RepeatedGcHook::<H, false>(
			hook,
			Arc::downgrade(&keep_alive),
		));
	}
}

pub fn collect_garbage<I>(locks: I)
where
	I: IntoIterator<Item = (BorrowMutability, UserLock)>,
{
	let locks = locks.into_iter().collect::<Vec<_>>();
	let (free_sessions, total_session_count) = MovableSessionGuard::acquire_free();

	assert_eq!(
		free_sessions.len(),
		total_session_count,
		"No sessions may be alive at the time `collect_garbage` is called."
	);

	// Run finalizers
	let mut free_sessions_2 = Vec::with_capacity(free_sessions.len());

	for free_session in free_sessions {
		let session = free_session.make_local();
		session.handle().acquire_locks(locks.iter().copied());

		unsafe {
			// Safety: the destructor list of this session could not have grown since we acquired it
			// at the top of this function and we already ensured that all sessions that could
			// possibly observe a dead handle would be dead themselves.
			SessionStateGcManager::get(session.handle())
				.finalizers
				.process_once(session.handle());
		}

		free_sessions_2.push(session.make_movable());
	}

	// Run compactors
	for free_session in free_sessions_2 {
		let session = free_session.make_local();
		session.handle().acquire_locks(locks.iter().copied());

		unsafe {
			// Safety: see above
			SessionStateGcManager::get(session.handle())
				.compactors
				.process_once(session.handle());
		}

		// Technically not necessary but allows us to fail more predictably.
		drop(session.make_movable());
	}
}
