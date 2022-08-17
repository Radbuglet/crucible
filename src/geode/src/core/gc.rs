use ahash::RandomState;
use crucible_core::cell::UnsafeCellExt;
use std::{any::Any, cell::UnsafeCell, collections::HashMap, hash};

use super::session::{Session, StaticStorageGetter, StaticStorageHandler};

#[derive(Default)]
pub(crate) struct SessionStateGcManager {
	hooks: HashMap<HashableDctorFn, Box<dyn Any + Send>, RandomState>,
}

impl StaticStorageHandler for SessionStateGcManager {
	type Comp = UnsafeCell<Self>;

	fn init_comp(comp: &mut Option<Self::Comp>) {
		if comp.is_none() {
			*comp = Some(Default::default());
		}
	}
}

struct HashableDctorFn(unsafe fn(Session, &mut (dyn Any + Send)));

impl hash::Hash for HashableDctorFn {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		(self.0 as usize).hash(state);
	}
}

impl Eq for HashableDctorFn {}

impl PartialEq for HashableDctorFn {
	fn eq(&self, other: &Self) -> bool {
		(self.0 as usize) == (other.0 as usize)
	}
}

impl Session<'_> {
	// TODO: Optimize
	pub fn register_gc_hook<H: GcHook>(self, hook: H) {
		unsafe fn process_many<H: GcHook>(session: Session, targets: &mut (dyn Any + Send)) {
			let targets = targets.downcast_mut::<Vec<H>>().unwrap();

			for target in targets.drain(..) {
				target.process(session);
			}
		}

		let state = unsafe { SessionStateGcManager::get(self).get_mut_unchecked() };

		let entry = state
			.hooks
			.entry(HashableDctorFn(process_many::<H>))
			.or_insert_with(|| Box::new(Vec::<H>::new()));

		entry.downcast_mut::<Vec<H>>().unwrap().push(hook);
	}
}

pub unsafe trait GcHook: 'static + Sized + Send {
	unsafe fn process(self, session: Session);
}
