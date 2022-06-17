use bumpalo::Bump;
use std::mem::ManuallyDrop;

#[derive(Debug, Default)]
pub struct LeakyBump {
	bump: ManuallyDrop<Bump>,
}

impl LeakyBump {
	pub fn alloc<T>(&self, value: T) -> &'static T {
		let ptr = self.bump.alloc(value);
		let ptr = unsafe {
			// Safety: we can leave this unbounded because the `Bump` will never be dropped.
			&*(ptr as *const T)
		};
		ptr
	}
}
