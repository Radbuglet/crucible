use once_cell::sync::OnceCell;
use std::alloc::Layout;
use std::ptr::{null_mut, NonNull};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Mutex;

#[derive(Debug)]
pub struct Bump {
	// A mutex which we lock while we grow the bump
	work_lock: OnceCell<Mutex<()>>,

	// An offset from `heap_start` to the write head, which indicates the address to which we'll
	// commit our next allocation.
	heap_write_offset: AtomicUsize,

	// A pointer to the start of the current page
	heap_start: *mut u8,

	// A pointer to the footer of the current page
	heap_footer: *mut BumpPage,

	// Page layout
	page_layout: Layout,
}

unsafe impl Send for Bump {}
unsafe impl Sync for Bump {}

impl Bump {
	pub const fn new(page_layout: Layout) -> Self {
		Self {
			work_lock: OnceCell::new(),
			heap_write_offset: AtomicUsize::new(0),
			heap_start: null_mut(),
			heap_footer: null_mut(),
			page_layout,
		}
	}

	pub fn alloc(&self, layout: Layout) -> NonNull<()> {
		//         let old_offset = self.heap_write_offset.fetch_update(set_order, fetch_order, || {
		//
		//         });
		//         let heap_head = self.heap_start.wrapping_add(
		//
		//         );
		//
		//         self.heap_write_offset.
		todo!()
	}

	pub fn pop_page(&mut self) {
		todo!()
	}
}

impl Drop for Bump {
	fn drop(&mut self) {
		todo!()
	}
}

struct BumpPage {
	write_head: *const (),
	prev: Option<NonNull<BumpPage>>,
}
