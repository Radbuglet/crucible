use super::{
	lock::{BorrowMutability, UserLock},
	session::{MovableSessionGuard, Session, StaticStorageGetter, StaticStorageHandler},
};

// === Internals === //

mod internal {
	use std::{
		any,
		cell::UnsafeCell,
		mem::{self, MaybeUninit},
		ptr,
	};

	use bumpalo::Bump;
	use crucible_core::cell::UnsafeCellExt;

	use crate::core::session::Session;

	#[derive(Default)]
	pub struct FinalizerExecutor {
		bump: UnsafeCell<Bump>,
	}

	#[repr(C)]
	struct FinalizerEntry<T> {
		header: FinalizerHeader,
		value: MaybeUninit<T>,
	}

	struct FinalizerHeader {
		handler: unsafe fn(Session, *mut Self) -> *mut Self,
	}

	impl FinalizerExecutor {
		pub fn push<H: GcHook>(&self, entry: H) {
			unsafe fn handler<H: GcHook>(
				session: Session,
				base: *mut FinalizerHeader,
			) -> *mut FinalizerHeader {
				let base = base.cast::<FinalizerEntry<H>>();
				let entry = ptr::addr_of!((*base).value).cast::<H>().read();
				entry.process(session);

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
			bump.alloc(FinalizerEntry {
				header: FinalizerHeader {
					handler: handler::<H>,
				},
				value: MaybeUninit::new(entry),
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

	pub trait GcHook: 'static + Sized + Send {
		unsafe fn process(self, session: Session);
	}
}

// === Global state === //

#[derive(Default)]
pub(crate) struct SessionStateGcManager {
	exec: internal::FinalizerExecutor,
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

pub use internal::GcHook;

impl Session<'_> {
	pub unsafe fn register_gc_hook<H: GcHook>(self, hook: H) {
		SessionStateGcManager::get(self).exec.push(hook);
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

	for free_session in free_sessions {
		let session = free_session.make_local();
		session.handle().acquire_locks(locks.iter().copied());

		unsafe {
			// Safety: the destructor list of this session could not have grown since we acquired it
			// at the top of this function and we already ensured that all sessions that could
			// possibly observe a dead handle would be dead themselves.
			SessionStateGcManager::get(session.handle())
				.exec
				.process_once(session.handle());
		}

		// Technically not necessary but allows us to fail early.
		drop(session.make_movable());
	}
}
