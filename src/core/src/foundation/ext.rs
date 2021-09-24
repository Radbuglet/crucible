use crate::foundation::{LazyProviderExt, Provider, ProviderExt, RwLock, RwLockManager};

pub trait ProviderRwLockExt: Provider {
	fn init_lock<T: 'static>(&self, value: T);
	fn get_lock<T: 'static>(&self) -> &RwLock<T>;
}

impl<Target: ?Sized + Provider> ProviderRwLockExt for Target {
	fn init_lock<T: 'static>(&self, value: T) {
		self.init(RwLock::new(self.get::<RwLockManager>().clone(), value))
	}

	fn get_lock<T: 'static>(&self) -> &RwLock<T> {
		self.get::<RwLock<T>>()
	}
}
