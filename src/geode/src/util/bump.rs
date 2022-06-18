use super::cell::OnlyMut;
use bumpalo::Bump;
use std::mem::ManuallyDrop;

#[derive(Debug, Default)]
pub struct LeakyBump {
	bump: ManuallyDrop<OnlyMut<Bump>>,
}

impl LeakyBump {
	pub fn alloc<T>(&mut self, value: T) -> &'static T {
		let ptr = self.bump.get().alloc(value);
		let ptr = unsafe {
			// Safety: we can leave this unbounded because the `Bump` will never be dropped.
			&*(ptr as *const T)
		};
		ptr
	}
}
