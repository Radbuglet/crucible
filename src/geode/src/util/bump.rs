use bumpalo::Bump;
use crucible_core::sync::AssertSync;
use std::mem::ManuallyDrop;

#[derive(Debug, Default)]
pub struct LeakyBump {
	bump: ManuallyDrop<AssertSync<Bump>>,
}

impl LeakyBump {
	pub fn alloc<T>(&mut self, value: T) -> &'static T {
		let ptr = self.bump.get_mut().alloc(value);
		unsafe {
			// Safety: we can leave this unbounded because the `Bump` will never be dropped.
			&*(ptr as *const T)
		}
	}
}
