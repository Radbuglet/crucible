#![feature(allocator_api)]

use bumpalo::Bump;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

fn main() {
	let start = GLOBAL.bytes_allocated();
	let bump = Bump::with_capacity(128);
	bump.alloc(4);
	let end = GLOBAL.bytes_allocated();

	println!("{start} to {end} (count {})", end - start);
	dbg!(std::mem::size_of::<HashMap<u32, u32>>());
	dbg!(std::mem::size_of::<Vec<u32>>());
	dbg!(std::mem::size_of::<String>());
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc {
	count: AtomicUsize::new(0),
};

struct CountingAlloc {
	count: AtomicUsize,
}

impl CountingAlloc {
	pub fn bytes_allocated(&self) -> usize {
		self.count.load(Ordering::Relaxed)
	}
}

unsafe impl GlobalAlloc for CountingAlloc {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		self.count.fetch_add(layout.size(), Ordering::Relaxed);
		System.alloc(layout)
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		self.count.fetch_sub(layout.size(), Ordering::Relaxed);
		System.dealloc(ptr, layout);
	}
}
