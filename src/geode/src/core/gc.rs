use super::session::{MovableSessionGuard, Session, StaticStorageGetter, StaticStorageHandler};

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

	#[cfg(test)]
	mod tests {
		use std::sync::{
			atomic::{AtomicUsize, Ordering as AtomicOrdering},
			Arc,
		};

		use crate::core::session::LocalSessionGuard;

		use super::*;

		#[test]
		fn ensure_all_finalized() {
			// Definitions
			let finalized = Arc::new(AtomicUsize::new(0_usize));

			struct Task1(usize, Arc<AtomicUsize>);

			impl GcHook for Task1 {
				unsafe fn process(self, _: Session) {
					self.1.fetch_add(self.0, AtomicOrdering::Relaxed);
				}
			}

			struct Task2(u8, Arc<AtomicUsize>);

			impl GcHook for Task2 {
				unsafe fn process(self, _: Session) {
					self.1.fetch_add(self.0.into(), AtomicOrdering::Relaxed);
				}
			}

			// Test
			let session = LocalSessionGuard::new();
			let s = session.handle();

			let finalizers = FinalizerExecutor::default();
			for i in 0..=1000 {
				finalizers.push(Task1(i, finalized.clone()));
			}

			for i in 0..=0xFF {
				finalizers.push(Task2(i, finalized.clone()));
			}

			unsafe {
				finalizers.process_once(s);
			}
			assert_eq!(finalized.load(AtomicOrdering::Relaxed), 500500 + 32640);
		}
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

pub fn collect_garbage() {
	let (free_sessions, mut total_session_count) = MovableSessionGuard::acquire_free();
	let mut free_sessions = free_sessions.map(Some).collect::<Vec<_>>();

	loop {
		// === Scan for a session === //

		// We want to make sure that all sessions that existed at the same time as our `free_sessions` are
		// destroyed as well, since doing so ensures that all references that could potentially point to
		// finalization targets will have expired.
		//
		// We can achieve this in a somewhat conservative manner by ensuring that the list of sessions we
		// acquire is equal to the total number of sessions. Even if another thread decides to create a
		// new session immediately after we acquire the free sessions, we know this session couldn't
		// possibly point to targets finalized in these sessions because they were already marked as dead.
		assert_eq!(
			free_sessions.len(),
			total_session_count,
			"All session guards in the application must be destroyed before returning control to the garbage collector."
		);

		// TODO
	}
}
