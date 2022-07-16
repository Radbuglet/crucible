use parking_lot::{lock_api::RawMutex, Mutex};

pub const fn new_lot_mutex<T>(value: T) -> Mutex<T> {
	Mutex::const_new(RawMutex::INIT, value)
}
