use std::cell::Cell;
use std::rc::Rc;

pub trait CellExt {
    type Inner;

    fn clone_inner(&self) -> Self::Inner;
}

impl<T: ?Sized> CellExt for Cell<std::rc::Rc<T>> {
    type Inner = std::rc::Rc<T>;

    fn clone_inner(&self) -> Self::Inner {
        let rc = unsafe {
            // Safety: Cell's semantics mean that its contents will never have a mutable reference to
            // its inner contents during safe code invocation given an immutable reference, since that
            // would break access invariants.
            &*self.as_ptr()
        };

        Rc::clone(rc)
    }
}
