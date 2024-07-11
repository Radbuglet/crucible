use core::fmt;
use std::cell::UnsafeCell;

trait Anything {}

impl<T: ?Sized> Anything for T {}

#[derive(Default)]
pub struct DropBump<'a> {
    bump: bumpalo::Bump,
    entries: UnsafeCell<Vec<*mut (dyn Anything + 'a)>>,
}

impl fmt::Debug for DropBump<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DropBump").finish_non_exhaustive()
    }
}

impl<'p> DropBump<'p> {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::mut_from_ref)]
    pub fn alloc<'a, T: 'p>(&'a self, val: T) -> &'a mut T {
        let val = self.bump.alloc(val);
        unsafe {
            // Safety: we're non-reentrant and this structure is `!Sync`
            (*self.entries.get()).push(val)
        }
        val
    }
}

impl Drop for DropBump<'_> {
    fn drop(&mut self) {
        for &mut entry in self.entries.get_mut() {
            unsafe {
                // Safety: All references into the bump are tied to immutable references to this
                // structure so they have all expired. We know these objects are still valid to
                // access since we effectively pretend as if we're holding references to them
                // and drop-check validates the rest.
                entry.drop_in_place()
            };
        }
    }
}

mod doc_tests {
    /// ```compile_fail
    /// use crucible_utils::lifetimes::DropBump;
    ///
    /// struct Inspector<'a>(&'a Box<u32>);
    ///
    /// impl Drop for Inspector<'_> {
    ///     fn drop(&mut self) {
    ///         eprintln!("The value is {:?}", self.0);
    ///     }
    /// }
    ///
    /// fn test() {
    ///     let bump = DropBump::new();
    ///     let value = bump.alloc(Box::new(1u32));
    ///     let inspector = bump.alloc(Inspector(&value));
    /// }
    /// ```
    fn _dc_1() {}

    /// ```
    /// use crucible_utils::lifetimes::DropBump;
    ///
    /// struct Inspector<'a>(&'a Box<u32>);
    ///
    /// impl Drop for Inspector<'_> {
    ///     fn drop(&mut self) {
    ///         eprintln!("The value is {:?}", self.0);
    ///     }
    /// }
    ///
    /// let value = Box::new(1u32);
    /// let bump = DropBump::new();
    /// let inspector = bump.alloc(Inspector(&value));
    /// ```
    fn _dc_2() {}
}
