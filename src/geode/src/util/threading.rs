use parking_lot::{lock_api::RawMutex, Mutex};

pub const fn new_lot_mutex<T>(value: T) -> Mutex<T> {
	Mutex::const_new(RawMutex::INIT, value)
}

// pub struct HandlerList<T> {
// 	active_set: Mutex<Vec<T>>,
// 	next_set: Mutex<Vec<T>>,
// }
//
// impl<T> Default for HandlerList<T> {
// 	fn default() -> Self {
// 		Self::new()
// 	}
// }
//
// impl<T> HandlerList<T> {
// 	pub const fn new() -> Self {
// 		Self {
// 			active_set: new_lot_mutex(Vec::new()),
// 			next_set: new_lot_mutex(Vec::new()),
// 		}
// 	}
//
// 	pub fn add(&self, handle: T) {
// 		if let Some(mut main_set) = self.active_set.try_lock() {
// 			main_set.push(handle);
// 		} else {
// 			self.next_set.lock().push(handle);
// 		}
// 	}
//
// 	pub fn flush<F>(&self, mut handler: F)
// 	where
// 		F: FnMut(&mut T, bool) -> bool,
// 	{
// 		// Handle the main list.
// 		let mut active_list = self.active_set.lock();
// 		active_list.retain_mut(|elem| handler(elem, true));
//
// 		loop {
// 			// Copy elements from the new list to the active list.
// 			let mut new_list = self.next_set.lock();
//
// 			// If there's nothing else to run,
// 			if new_list.is_empty() {
// 				break;
// 			}
//
// 			active_list.extend(active_list.drain(..));
//
// 			// Release the `new_list` so user code can call `.add()` without causing a dead-lock.
// 			drop(new_list);
//
// 			//
// 		}
// 	}
// }
//
// pub struct HandlerListIter<'a, T> {
// 	active_list_mutex: MutexGuard<'a, Vec<T>>,
// 	next_set_mutex: &'a Mutex<Vec<T>>,
// }
