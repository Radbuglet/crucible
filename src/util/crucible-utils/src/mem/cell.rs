use std::cell::RefCell;

use derive_where::derive_where;

#[derive(Debug)]
#[derive_where(Default)]
pub struct CellVec<T>(RefCell<Vec<T>>);

impl<T> CellVec<T> {
    pub const fn new() -> Self {
        Self(RefCell::new(Vec::new()))
    }

    pub const fn wrap(vec: Vec<T>) -> Self {
        Self(RefCell::new(vec))
    }

    pub fn push(&self, value: T) {
        self.0.borrow_mut().push(value);
    }

    pub fn finish(self) -> Vec<T> {
        self.0.into_inner()
    }
}
