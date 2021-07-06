#![allow(dead_code)]  // To suppress warnings while developing.

//! A simplified (and less optimized) version of hashbrown's [RawTable] that allows us to implement
//! lazy deletion.
//!
//! [RawTable]: https://github.com/rust-lang/hashbrown/blob/805b5e28ac7b12ad901aceba5ee641de50c0a3d1/src/raw/mod.rs

use std::alloc::{Allocator, AllocError, Layout};
use std::ptr::NonNull;
use std::marker::PhantomData;
use std::mem;

use crate::inner::ub::{add_ptr, sub_ptr, unwrap_unchecked};

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct Ctrl(u8);

impl Ctrl {
    pub const EMPTY: Self = Self(0b_1000_0000);
    pub const DELETED: Self = Self(0b_1111_1111);

    pub fn as_byte(self) -> u8 {
        self.0
    }
}

pub struct Group(usize);

impl Group {
    pub const ALIGN: usize = mem::align_of::<Self>();
    pub const WIDTH: usize = mem::size_of::<Self>();
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ReserveError {
    AllocError(AllocError),
    LayoutError,
}

pub struct RawTable<T, A: Allocator> {
    /// Tells Rust:
    ///
    /// - Type (lifetime) variance is allowed.
    /// - `T` must **strictly** outlive `Self`, ensuring that we don't access invalid data during
    ///   `Drop`.
    ///
    _ty: PhantomData<T>,

    /// A pointer to the first control byte of the table.
    ///
    /// ## Layout
    ///
    /// ```txt
    /// [Padding] [T1] [T2] [...] [TN] [C1] [C2] [...] [CN] [C1Mirror]
    /// ^ allocation starts here       ^ ctrl points here
    /// ```
    ///
    /// - The first `ctrl` byte is aligned to the `CtrlGroup` alignment boundary.
    /// - There is no padding between the last bucket and the first control byte. This alignment is
    ///   achieved by taking the LCD of the two values' alignments.
    /// - The entire allocation has a size of less or equal to than `isize::MAX`.
    ///   This means that every byte in the table (and one past the end) will be accessible from the
    ///   start of the table.
    ///
    /// ## About Empty Singletons
    ///
    /// - The table is an empty singleton IFF `is_empty_singleton` returns `true`. In all other
    ///   cases, the table is backed by a heap allocation.
    ///
    /// - The empty singleton is the only table with `0` buckets.
    ///
    /// - The memory of the singleton is immutable.
    ///
    /// - `bucket_mask` is `0` when the table is an empty singleton, even though this would yield
    ///   a bucket count of `1`. Do not query `buckets` when the table is an empty singleton.
    ///
    ctrl: NonNull<Ctrl>,

    /// The number of buckets in the table minus one. Can be bitwise ANDed with a hash to obtain
    /// a probe start index.
    ///
    /// ## Invariants
    ///
    /// - Always less than or equal to `isize::MAX` (this can usually be merged with the allocation
    ///   size check).
    /// - Must be a power of two for the bitwise AND trick to work.
    /// - For tables with zero actual buckets, this value will be `0`.
    ///
    bucket_mask: usize,

    /// The number of `EMPTY` slots that can be filled before we need to grow the table.
    ///
    /// This enforces both probing invariants and load factor.
    growth_left: usize,

    alloc: A,
}

#[repr(C)]
struct DummyTable {
    _align: [Group; 0],

    // Done this way since we cannot construct SSE values in `const`.
    ctrl: [Ctrl; Group::WIDTH],
}

impl<T, A: Allocator> RawTable<T, A> {
    // === Creation === //

    const EMPTY_SINGLETON: DummyTable = DummyTable {
        _align: [],
        ctrl: [Ctrl::EMPTY; Group::WIDTH],
    };

    const EMPTY_SINGLETON_CTRL: NonNull<Ctrl> = unsafe {
        NonNull::new_unchecked(&Self::EMPTY_SINGLETON.ctrl as *const _ as *mut Ctrl)
    };

    /// Creates a new table without heap backed buckets.
    pub const fn new(alloc: A) -> Self {
        Self {
            _ty: PhantomData,
            ctrl: Self::EMPTY_SINGLETON_CTRL,
            bucket_mask: 0,
            growth_left: 0,
            alloc,
        }
    }

    /// Creates an empty table with the specified bucket count. Returns the empty singleton when
    /// `buckets` is `0`.
    pub fn new_sized(capacity: usize, alloc: A) -> Result<Self, ReserveError> {
        if capacity == 0 {
            Ok (Self::new(alloc))
        } else {
            let table = unsafe {
                // Safety: we initialize the table before giving it to the user.
                Self::new_uninit(capacity, alloc)?
            };

            unsafe {
                // Safety: `ctrl` is safe for writes of this size. We are allowed to write individual
                // bytes like this because `Ctrl` is `#[repr(transparent)]` for `u8`.
                table.ctrl.as_ptr().write_bytes(
                    Ctrl::EMPTY.as_byte(),
                    table.ctrl_groups()
                );
            };

            Ok (table)
        }
    }

    /// Attempts to allocate a new table with a given bucket count but does not initialize the control
    /// byte array.
    ///
    /// ## Safety
    ///
    /// It is up to the consumer to properly initialize the control groups before use.
    ///
    /// The user must not request a bucket count of `0` because this breaks `is_empty_singleton`.
    ///
    unsafe fn new_uninit(capacity: usize, alloc: A) -> Result<Self, ReserveError> {
        debug_assert!(capacity != 0);

        let buckets = Self::capacity_to_buckets(capacity)
            .ok_or(ReserveError::LayoutError)?;

        let (layout, ctrl_offset) = Self::layout_for(buckets)
            .ok_or(ReserveError::LayoutError)?;

        let heap_ptr = alloc.allocate(layout)
            .map_err(|err| ReserveError::AllocError(err))?;

        // Safety: `ctrl_offset` is less than the table size, and is thus less than `isize::MAX`.
        // `ctrl_offset` is within the allocation's bounds, meaning that the pointer will be
        // non-null.
        let ctrl = add_ptr(heap_ptr.cast::<Ctrl>(), ctrl_offset);

        Ok (Self {
            _ty: PhantomData,
            ctrl,
            bucket_mask: buckets - 1,
            growth_left: capacity,
            alloc,
        })
    }

    /// TODO
    unsafe fn dealloc_no_drop(&mut self) {
        // Safety: for non empty singletons, we've already called `layout_for` with the
        // bucket count successfully, making subsequent calls also work.
        let (layout, ctrl_offset) = unwrap_unchecked(Self::layout_for(self.buckets()));

        // Safety: as per `layout_for` invariants, `ctrl_offset` is less than `isize::MAX`
        // and remains within the heap allocation's bounds.
        let heap_ptr = sub_ptr(self.ctrl.cast::<u8>(), ctrl_offset);

        // Safety: we are the sole owners of this allocation. It is up to users consuming
        // raw table pointers to limit their lifetimes and prevent dangling.
        //
        // Since this object can only be dropped once, `heap_ptr` must be currently
        // allocated.
        //
        // We used the exact same layout to allocate and deallocate this object.
        self.alloc.deallocate(heap_ptr, layout)
    }

    /// Generates a layout and a `ctrl` pointer offset (or `None` on overflow) given a bucket count.
    ///
    /// ## Invariants
    ///
    /// Both the `ctrl_offset`, and the `Layout`'s size will be less than `isize::MAX`. This function
    /// also has the intentional side effect of limiting `buckets` to `isize::MAX - Group::WIDTH`.
    ///
    fn layout_for(buckets: usize) -> Option<(Layout, usize)> {
        // The LCD of value alignments for the bucket and the control group. This is possible because
        // all memory alignments are powers of two.
        let align = usize::max(
            mem::align_of::<T>(),
            Group::ALIGN,
        );

        // Create an array of buckets.
        let ctrl_offset = (mem::size_of::<T>() as isize).checked_mul(buckets as isize)?;

        // This trick works by: a) incrementing the value by an offset guaranteed to increment its
        // quotient (unless already aligned) and b) dropping the lowest bits to ensure proper alignment.
        // We can use this bit hack because `align` is a power of two.
        let ctrl_offset = ctrl_offset.checked_add(align as isize - 1)? & !(align as isize - 1);

        // Add the control byte array.
        let size = ctrl_offset.checked_add(buckets as isize)?.checked_add(Group::WIDTH as isize)?;

        Some((
            unsafe { Layout::from_size_align_unchecked(size as usize, align) },
            ctrl_offset as usize
        ))
    }

    /// Fetches the number of buckets required to house a specified capacity. The returned value will
    /// be at least one larger than the passed capacity.
    ///
    /// Returns `None` if the bucket count is too big. (Note: this by itself does not enforce size
    /// invariants)
    fn capacity_to_buckets(capacity: usize) -> Option<usize> {
        if capacity < 8 {
            Some(capacity + 1)
        } else {
            Some(capacity.checked_mul(8)?.checked_div(7)?)
        }
    }

    // === Querying === //

    /// Returns whether or not we're an empty singleton.
    ///
    /// Empty singletons are the only table to have a bucket count of `0` and their memory must
    /// not be mutated.
    pub fn is_empty_singleton(&self) -> bool {
        self.ctrl == Self::EMPTY_SINGLETON_CTRL
    }

    /// Returns the number of buckets in the table. Assumes a non empty singleton.
    pub fn buckets(&self) -> usize {
        debug_assert!(!self.is_empty_singleton());
        self.bucket_mask + 1
    }

    /// Returns the number of buckets in the table, corrected for empty singletons.
    pub fn buckets_corrected(&self) -> usize {
        if self.is_empty_singleton() {
            0
        } else {
            self.bucket_mask + 1
        }
    }

    /// Returns the number of control groups in the table. Assumes a non-zero bucket count.
    pub fn ctrl_groups(&self) -> usize {
        self.buckets() + Group::WIDTH
    }

    /// TODO
    unsafe fn ctrl(&self, index: usize) -> NonNull<Ctrl> {
        debug_assert!(index <= self.buckets_corrected());
        add_ptr(self.ctrl, index)
    }

    /// TODO
    unsafe fn bucket(&self, index: usize) -> NonNull<T> {
        debug_assert!(index < self.buckets_corrected());
        sub_ptr(self.ctrl.cast::<T>(), index + 1)
    }

    /// TODO
    unsafe fn set_ctrl(&mut self, index: usize, value: Ctrl) {
        todo!()
    }

    // === Probing === //

    // TODO
}
