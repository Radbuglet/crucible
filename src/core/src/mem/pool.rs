use std::mem;

use parking_lot::Mutex;

use crate::{lang::polyfill::OptionPoly, mem::ptr::addr_of_ptr};

#[derive(Debug)]
pub struct GlobalPool<T: 'static> {
	pools: Mutex<Vec<Vec<T>>>,
}

impl<T> GlobalPool<T> {
	pub const fn new() -> Self {
		Self {
			pools: Mutex::new(Vec::new()),
		}
	}
}

#[derive(Debug)]
pub struct LocalPool<T: 'static> {
	global: Option<&'static GlobalPool<T>>,
	local_pool: Vec<T>,
	indebted_pool: Vec<T>,
}

impl<T: 'static> LocalPool<T> {
	pub const fn new() -> Self {
		Self {
			global: None,
			local_pool: Vec::new(),
			indebted_pool: Vec::new(),
		}
	}

	fn late_bind_global(&mut self, global: &'static GlobalPool<T>) {
		debug_assert!(
			self.global
				.p_is_none_or(|prev| addr_of_ptr(prev) == addr_of_ptr(global)),
			"acquired a `LocalPool` from multiple different `GlobalPools`"
		);

		self.global = Some(global);
	}

	pub fn acquire<F>(&mut self, global: &'static GlobalPool<T>, factory: F) -> T
	where
		F: FnOnce() -> Vec<T>,
	{
		// Late bind the global pool (constants cannot reference statics)
		self.late_bind_global(global);

		// Attempt to allocate from the local pool.
		if let Some(local) = self.local_pool.pop() {
			return local;
		}

		// Attempt to swap to our "indebted" pool.
		mem::swap(&mut self.local_pool, &mut self.indebted_pool);

		if let Some(local) = self.local_pool.pop() {
			return local;
		}

		// Attempt to steal a block from the global pool.
		let mut global_pool_blocks = global.pools.lock();

		if let Some(pool) = global_pool_blocks.pop() {
			// We drop the pool first because updating `local_pool` may cause a deallocation and we
			// want to minimize the time we hold a global lock.
			drop(global_pool_blocks);

			self.local_pool = pool;
			return self.local_pool.pop().unwrap();
		}

		// Otherwise, build a new block on this thread and use that.
		drop(global_pool_blocks); // We don't need the global queue anymore.

		self.local_pool = factory();
		self.local_pool.pop().unwrap()
	}

	pub fn release(&mut self, global: &'static GlobalPool<T>, block_size: usize, value: T) {
		assert_ne!(block_size, 0);
		self.late_bind_global(global);

		// Attempt to write to the main pool.
		if self.local_pool.len() < block_size {
			self.local_pool.push(value);
			return;
		}

		// Otherwise, prepare an indebted queue...
		self.indebted_pool.push(value);

		// ...and send it to the global pool once it fills up.
		if self.indebted_pool.len() >= block_size {
			let pool = mem::replace(&mut self.indebted_pool, Vec::new());
			global.pools.lock().push(pool);
		}
	}
}

impl<T: 'static> Drop for LocalPool<T> {
	fn drop(&mut self) {
		// Fetch the global pool if it was ever bound to this pool.
		let Some(global) = self.global else {
			// If no global pool was ever bound to this pool, our local pools are entirely empty and
			// thus wouldn't need to be saved anyways.
			return;
		};

		// Register any non-empty pools into the global pool.
		let mut pools = global.pools.lock();

		if !self.local_pool.is_empty() {
			pools.push(mem::replace(&mut self.local_pool, Vec::new()));
		}

		if !self.indebted_pool.is_empty() {
			pools.push(mem::replace(&mut self.indebted_pool, Vec::new()));
		}
	}
}
