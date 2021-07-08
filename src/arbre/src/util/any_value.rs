use std::{mem, mem::ManuallyDrop};

/// Stores any value that's an "allocation sub-type" of `H`.
///
/// An "allocation sub-type" is a type that:
/// - Has less alignment requirements than the parent type `H`.
/// - Has smaller size requirements than the parent type `H`.
///
/// `AnyObj` will also pick up certain characteristics of the sub type. For example, if `H` implements
/// `Copy`, `AnyObj` will also be `Copy`—even if the actual value doesn't implement `Copy`. These
/// derivations are never unsafe on their own, but they may impact soundness proofs for later `get_xx`
/// method calls.
// We abuse a quirk in the Rust layout system where the unused portion of a union can accept any bit
// pattern, including those imbued with pointer providence. This seems to be blessed by the Rust
// developers because MaybeUninit works this way as well.
#[repr(C)]
pub union AnyValue<H> {
    zst: (),
    value: ManuallyDrop<H>,
}

/// Poor man's transmute that:
/// - Doesn't check sizes at compile time.
/// - Can be used in the CTFE.
/// - Is super unsafe.
const unsafe fn bad_transmute<A, B>(value: A) -> B {
    #[repr(C)]
    union Transmute<A, B> { a: ManuallyDrop<A>, b: ManuallyDrop<B> }

    ManuallyDrop::into_inner(Transmute { a: ManuallyDrop::new(value) }.b)
}

impl<H> AnyValue<H> {
    /// Constructs an `AnyObj` without any initialized value.
    /// Users can initialize this value by writing through `as_mut_ptr`.
    pub const fn empty() -> Self {
        Self { zst: () }
    }

    pub const fn new<T>(value: T) -> Self {
        // === Check validity of passed value `T` (should be elided at compile time)
        if mem::size_of::<T>() > mem::size_of::<H>() {
            panic!("Type has a larger size than its container type.");
        }

        // All alignments are powers-of-two so larger alignments are guaranteed to be multiples of smaller alignments.
        if mem::align_of::<T>() > mem::align_of::<H>() {
            panic!("Type has stronger alignment requirements than its container type.");
        }

        // === Construct object
        // FIXME: Aneurysms (this would be less morally deficient if the CTFE supported `write`)

        // We create an `AnyObj` with active variants "value"
        let obj = AnyValue::<T> { value: ManuallyDrop::new(value) };

        // Since the Rust Abstract Machine(TM) doesn't track union variants, there's an implicit coercion
        // to the "zst" active variant. Thus, while transmuting from `T` to `H` may not be legal, transmuting
        // from `AnyObj<T> { value: ... }` to `AnyObj<H> { zst: ... }` should be.
        unsafe { bad_transmute(obj) }
    }

    pub const fn as_ptr<T>(&self) -> *const T {
        (self as *const Self).cast()
    }

    pub const fn as_mut_ptr<T>(&mut self) -> *mut T {
        (self as *mut Self).cast()
    }

    pub const unsafe fn get_ref<T>(&self) -> &T {
        &*self.as_ptr::<T>()
    }

    pub const unsafe fn get_mut<T>(&mut self) -> &mut T {
        &mut *self.as_mut_ptr::<T>()
    }

    pub unsafe fn get<T>(self) -> T {
        self.as_ptr::<T>().read()
    }
}

impl<H: Copy> Copy for AnyValue<H> {}
impl<H> Clone for AnyValue<H> {
    fn clone(&self) -> Self {
        let mut obj = Self::empty();
        unsafe { obj.as_mut_ptr::<H>().copy_from(self.as_ptr(), 1) };
        obj
    }
}
