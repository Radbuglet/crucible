use std::{fmt, hash};
use std::marker::{PhantomData, Unsize};
use std::ops::{CoerceUnsized, Deref};
use std::ptr::NonNull;

use crate::inner::ub::unchecked_index_mut;
use crate::weak::Weak;

// TODO: Code review.

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct BitSz {
    pub bits: u8,
    pub max: u64,
    pub next_field: u64,
}

impl BitSz {
    pub const fn new(bits: u8) -> Self {
        Self {
            bits,
            max: Self::max_val_for_bits(bits),
            next_field: Self::next_field_delta(bits),
        }
    }

    pub const fn max_val_for_bits(bits: u8) -> u64 {
        (1u64 << bits) - 1
    }

    pub const fn next_field_delta(bits: u8) -> u64 {
        1u64 << (bits + 1)
    }
}

/// A u64-sized representation of both a `Slot` and an `RcLoc`.
///
/// ## Layout
///
/// ```txt
/// [slot_or_rc] [gen]
/// ```
///
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
struct CompositeVal(u64);

impl CompositeVal {
    pub const GEN_FIELD: BitSz = BitSz::new(42);
    pub const SLOT_OR_RC_FIELD: BitSz = BitSz::new(64 - Self::GEN_FIELD.bits);

    pub fn compose(gen: u64, slot_or_rc: u64) -> Self {
        debug_assert!(gen <= Self::GEN_FIELD.max);
        debug_assert!(slot_or_rc <= Self::SLOT_OR_RC_FIELD.max as u64);

        Self (gen + (slot_or_rc << Self::GEN_FIELD.bits))
    }

    pub fn gen(self) -> u64 {
        self.0 & Self::GEN_FIELD.max
    }

    pub fn slot_or_rc(self) -> u64 {
        self.0 >> Self::GEN_FIELD.bits
    }
}

impl fmt::Debug for CompositeVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompositeVal_bitfield")
            .field("gen", &self.gen())
            .field("slot_or_rc", &self.slot_or_rc())
            .finish()
    }
}

/// The maximum (exclusive) value of `gen`.
const GEN_MAX: u64 = CompositeVal::GEN_FIELD.max;

/// The maximum (exclusive) value of `slot`.
const SLOT_MAX: usize = CompositeVal::SLOT_OR_RC_FIELD.max as usize;

/// The maximum (exclusive) value of `rc`.
const RC_MAX: u64 = CompositeVal::SLOT_OR_RC_FIELD.max;

/// A u64-sized representation of a `Slot` in the ObjectDB.
///
/// ## Format
///
/// - If `gen == GEN_MAX`, the slot is empty.
/// - If `slot == SLOT_MAX && is_empty()`, the next empty slot is `None`.
/// - There are no limitations on `rc`.
///
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct Slot(CompositeVal);

impl Slot {
    pub fn new_full(gen: u64, rc: u64) -> Self {
        debug_assert!(gen != GEN_MAX);
        Self (CompositeVal::compose(gen, rc))
    }

    pub fn new_empty(next: Option<usize>) -> Self {
        match next {
            Some(next) => {
                debug_assert!(next != SLOT_MAX);
                Self (CompositeVal::compose(GEN_MAX, next as u64))
            }
            None => {
                Self (CompositeVal::compose(GEN_MAX, SLOT_MAX as u64))
            }
        }
    }

    pub fn is_full(self) -> bool {
        self.0.gen() != GEN_MAX
    }

    pub fn is_empty(self) -> bool {
        self.0.gen() == GEN_MAX
    }

    pub fn matches(self, loc: RcLoc) -> bool {
        // Since `loc.0.gen() != GEN_MAX` and `GEN_MAX` indicates an empty slot, this checks
        // for both empty slots and invalid generations.
        loc.0.gen() == self.0.gen()
    }

    pub fn inc_rc(self) -> Self {
        debug_assert!(self.is_full());

        let rc = self.rc() + 1;
        assert!(rc <= RC_MAX, "Failed to increment reference count (too many references!)");
        Self::new_full(self.0.gen(), rc)
    }

    pub fn dec_rc(self) -> Self {
        debug_assert!(self.is_full());
        debug_assert!(self.rc() > 0);

        Self::new_full(
            self.0.gen(),
            self.rc() - 1  // Already checked in debug builds.
        )
    }

    pub fn rc(self) -> u64 {
        debug_assert!(self.is_full());
        self.0.slot_or_rc()
    }

    pub fn next_empty(self) -> Option<usize> {
        debug_assert!(self.is_empty());

        let slot = self.0.slot_or_rc() as usize;
        if slot != SLOT_MAX {
            Some(slot)
        } else {
            None
        }
    }
}

/// A u64-sized representation of a `location` in the ObjectDB.
///
/// ## Format
///
/// - Gen (`gen != `GEN_MAX`)
/// - Slot (`slot != SLOT_MAX`)
///
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct RcLoc(CompositeVal);

impl RcLoc {
    pub fn new(gen: u64, slot: usize) -> Self {
        debug_assert!(gen != GEN_MAX);
        debug_assert!(slot != SLOT_MAX);

        Self (CompositeVal::compose(gen, slot as u64))
    }

    pub fn slot(self) -> usize {
        self.0.slot_or_rc() as usize
    }
}

struct World {
    /// The allocated slots.
    slots: Vec<Slot>,

    /// The first empty entity slot in the empty entity slot linked list.
    free_head: Option<usize>,

    /// The discriminator generator.
    gen: u64,
}

impl World {
    const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            gen: 0,
        }
    }

    /// Gets a mutable [WorldInner] reference.
    ///
    /// ## Safety
    ///
    /// `use_world` calls must never be nested. Users must not yield control to user code from within the
    /// handler unless it is though panic unwinding.
    unsafe fn run<F: FnOnce(&mut Self) -> R, R>(handler: F) -> R {
        use crate::inner::ub::ManualCell;

        thread_local! {
            static WORLD: ManualCell<World> = ManualCell::new(World::new());
        }

        WORLD.with(|world| {
            // Safety: provided by caller
            handler(&mut *world.borrow_mut())
        })
    }
}

/// Derives `Hash`, `Eq`, `PartialEq`, and `CoerceUnsized`. Delegates value checks to the specified
/// field.
macro_rules! derive_for_rc {
    ($target:ident, $id_field:tt) => {
        impl<T: ?Sized> hash::Hash for $target<T> {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.$id_field.hash(state)
            }
        }

        impl<T: ?Sized> Eq for $target<T> {}

        impl<T: ?Sized> PartialEq for $target<T> {
            fn eq(&self, other: &Self) -> bool {
                self.$id_field.eq(&other.$id_field)
            }
        }

        impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<$target<U>> for $target<T> {}
    };
}

pub struct WeakOrc<T: ?Sized> {
    /// Since the generation ID is only unique per thread and not throughout the entire application,
    /// we must prevent entities from crossing the thread boundary to allow `gen` to safely discriminate
    /// between entities.
    ///
    /// We use `PhantomData` with a known `!Send` type because `negative_impls` haven't yet been
    /// stabilized.
    _no_send: PhantomData<*const ()>,

    /// Tells the drop checker that we may access an instance of `T` during `Drop`.
    ///
    /// Covariance (in lifetimes specifically) is allowed here because users cannot obtain a sub-typed
    /// mutable reference without the help of `UnsafeCell<T>`, which is invariant over `T`.
    _ty: PhantomData<T>,

    /// The location of the reservation.
    loc: RcLoc,

    /// A pointer to the value. Only valid for accesses when the object is still registered in the
    /// database.
    ptr: NonNull<T>,
}

impl<T: ?Sized> WeakOrc<T> {
    pub fn try_upgrade(&self) -> Option<Orc<T>> {
        let can_upgrade = {
            let handler = move |world: &mut World| {
                let slot = unsafe {
                    // Safety: we never shrink the allocation array
                    unchecked_index_mut(&mut world.slots, self.loc.slot())
                };

                if slot.matches(self.loc) {
                    *slot = slot.inc_rc();
                    true
                } else {
                    false
                }
            };

            unsafe { World::run(handler) }
        };

        if can_upgrade {
            Some (Orc (*self))
        } else {
            None
        }
    }

    pub fn upgrade(&self) -> Orc<T> {
        self.try_upgrade().unwrap()
    }

    pub fn strong_count(&self) -> u64 {
        let handler = move |world: &mut World| {
            let slot = unsafe {
                // Safety: we never shrink the allocation array
                unchecked_index_mut(&mut world.slots, self.loc.slot())
            };

            if slot.matches(self.loc) {
                slot.rc()
            } else {
                0
            }
        };

        unsafe { World::run(handler) }
    }

    pub fn is_alive(&self) -> bool {
        let handler = move |world: &mut World| {
            let slot = unsafe {
                // Safety: we never shrink the allocation array
                unchecked_index_mut(&mut world.slots, self.loc.slot())
            };

            slot.matches(self.loc)
        };

        unsafe { World::run(handler) }
    }

    pub fn raw_ptr(&self) -> NonNull<T> {
        self.ptr
    }
}

derive_for_rc!(WeakOrc, loc);

impl<T: ?Sized> fmt::Debug for WeakOrc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WeakOrc")
            //.field("_no_send", &"PhantomData")
            //.field("_ty", &"PhantomData")
            .field("loc", &self.loc)
            .field("ptr", &self.ptr)
            .finish()
    }
}

impl<T: ?Sized> Copy for WeakOrc<T> {}

impl<T: ?Sized> Clone for WeakOrc<T> {
    fn clone(&self) -> Self {
        Self {
            _no_send: PhantomData,
            _ty: PhantomData,
            loc: self.loc,
            ptr: self.ptr,
        }
    }
}

impl<T: ?Sized> Weak for WeakOrc<T> {
    fn is_alive(&self) -> bool {
        self.is_alive()
    }
}

pub struct Orc<T: ?Sized>(WeakOrc<T>);

impl<T> Orc<T> {
    pub fn new(value: T) -> Self {
        let handler = move |world: &mut World| {
            // Generate a generation identifier
            let gen = {
                world.gen += 1;
                if world.gen >= GEN_MAX {
                    panic!("Too many generation identifiers requested! (Count: {})", GEN_MAX);
                }
                world.gen
            };

            // Reserve an appropriate slot
            let slot_data = Slot::new_full(gen, 1);
            let index = match world.free_head {
                Some(index) => {
                    let slot = unsafe {
                        // Safety: world maintains "free_head" as a valid index
                        unchecked_index_mut(&mut world.slots, index)
                    };
                    debug_assert!(slot.is_empty());

                    world.free_head = slot.next_empty();
                    *slot = slot_data;
                    index
                }
                None => {
                    if world.slots.len() >= SLOT_MAX {
                        panic!("Too many ObjectDB slots requested! (Count: {})", SLOT_MAX);
                    }
                    world.slots.push(slot_data);
                    world.slots.len() - 1
                }
            };
            let loc = RcLoc::new(gen, index);

            // Allocate value
            let value = Box::new(value);
            let ptr = NonNull::from(Box::leak(value));

            // Construct Rc.
            Self (WeakOrc {
                _no_send: PhantomData,
                _ty: PhantomData,
                loc, ptr,
            })
        };

        unsafe { World::run(handler) }
    }
}

impl<T: ?Sized> Orc<T> {
    pub fn downgrade(rc: &Self) -> WeakOrc<T> {
        rc.0
    }

    pub fn strong_count(rc: &Self) -> u64 {
        rc.0.strong_count()
    }

    pub fn is_alive(rc: &Self) -> bool {
        rc.0.is_alive()
    }

    pub fn raw_ptr(rc: &Self) -> NonNull<T> {
        rc.0.ptr
    }
}

derive_for_rc!(Orc, 0);

impl<T: ?Sized> fmt::Debug for Orc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Orc")
            .field(&self.0)
            .finish()
    }
}

impl<T: ?Sized> Deref for Orc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            // Safety: the box's contents are guaranteed to be alive until the last Orc is dropped.
            self.0.ptr.as_ref()
        }
    }
}

impl<T: ?Sized> Clone for Orc<T> {
    fn clone(&self) -> Self {
        // Increment Rc
        {
            let handler = move |world: &mut World| {
                let slot = unsafe {
                    // Safety: we never shrink the allocation array
                    unchecked_index_mut(&mut world.slots, self.0.loc.slot())
                };

                debug_assert!(slot.matches(self.0.loc));
                *slot = slot.inc_rc();
            };

            unsafe { World::run(handler) }
        }

        Self (self.0)
    }
}

impl<T: ?Sized> Drop for Orc<T> {
    fn drop(&mut self) {
        let should_drop = {
            let inner = self.0;

            let handler = move |world: &mut World| {
                let slot_index = inner.loc.slot();
                let slot = unsafe {
                    // Safety: we never shrink the allocation array
                    unchecked_index_mut(&mut world.slots, slot_index)
                };

                let decremented = slot.dec_rc();
                if decremented.rc() > 0 {
                    *slot = decremented;
                    false
                } else {
                    *slot = Slot::new_empty(world.free_head);
                    world.free_head = Some(slot_index);
                    true
                }
            };

            unsafe { World::run(handler) }
        };

        if should_drop {
            drop(unsafe {
                // Safety: we are the last Rc to the allocation, so we can take ownership and drop it.
                // No other weak references can be upgraded because we already condemned the reference.
                Box::from_raw(self.0.ptr.as_ptr())
            })
        }
    }
}

impl<T: ?Sized> Weak for Orc<T> {
    fn is_alive(&self) -> bool {
        true
    }
}
