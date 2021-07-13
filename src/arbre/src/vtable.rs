use std::marker::{PhantomData, Unsize};
use std::mem::MaybeUninit;
use std::ptr::{DynMetadata, Pointee, from_raw_parts, from_raw_parts_mut};

use crate::fetch::{Comp, RootCastTo};
use crate::key::{RawKey, Key};
use crate::util::{AnyValue, ConstVec, PhantomInvariant, unsize_meta};

// TABLE_CAP must be a prime number or the randomization algorithm will be *extremely* ineffective.
const TABLE_CAP: usize = 33;

// This value is hard coded in certain panic messages in this module, and need to be updated when
// modifying this const.
pub const MAX_COMPS: usize = 16;

// === Fields === //

#[derive(Copy, Clone)]
pub struct RawField {
    offset: usize,
    meta: AnyValue<DynMetadata<()>>,
}

impl RawField {
    pub const fn new<T>(offset: usize, meta: T) -> Self {
        Self {
            offset,
            meta: AnyValue::new(meta),
        }
    }

    pub const fn subfield(&self, field: &Self) -> Self {
        Self {
            offset: self.offset + field.offset,
            meta: field.meta,
        }
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub const unsafe fn meta<T: Copy>(&self) -> T {
        *self.meta.get_ref::<T>()
    }

    pub const unsafe fn typed<S: ?Sized, T: ?Sized + Pointee>(&self) -> Field<S, T> {
        Field::new_raw(self.offset, self.meta::<T::Metadata>())
    }
}

pub struct Field<S: ?Sized, T: ?Sized + Pointee> {
    container_ty: PhantomInvariant<S>,
    offset: usize,
    meta: T::Metadata,
}

pub macro get_field($struct:path, $field:ident) {
    unsafe {
        // Ensure that the type being queried is actually a struct, has a field named `$field`, and that
        // fetching this field will not cause a second user-controlled deref.
        let $struct { $field: _, .. };

        let base = ::std::mem::align_of::<$struct>() as *const $struct;
        let field = ::std::ptr::addr_of!((*base).$field) as *const _;
        Field::new_from_pointers(base, field)
    }
}

impl<S: Pointee<Metadata = ()>> Field<S, S> {
    pub const fn identity() -> Self {
        Self {
            container_ty: PhantomData,
            offset: 0,
            meta: (),
        }
    }
}

impl<S: ?Sized, T: Sized + Pointee> Field<S, T> {
    pub const fn cast<T2>(self) -> Field<S, T2>
    where
        T2: ?Sized + Pointee,
        T: Unsize<T2>,
    {
        Field {
            container_ty: PhantomData,
            offset: self.offset,
            meta: unsize_meta::<T, T2>()
        }
    }
}

impl<S: ?Sized, T: ?Sized + Pointee> Field<S, T> {
    pub const unsafe fn new_from_pointers(base: *const S, field: *const T) -> Self {
        Self {
            container_ty: PhantomData,
            offset: field.cast::<u8>().offset_from(base.cast::<u8>()) as usize,
            meta: field.to_raw_parts().1,
        }
    }

    pub const unsafe fn new_raw(offset: usize, meta: T::Metadata) -> Self {
        Self {
            container_ty: PhantomData,
            offset, meta
        }
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn meta(self) -> T::Metadata {
        self.meta
    }

    pub const fn as_raw(self) -> RawField {
        RawField::new(self.offset, self.meta)
    }

    pub const fn subfield<T2: ?Sized + Pointee>(self, field: Field<T, T2>) -> Field<S, T2> {
        Field {
            container_ty: PhantomData,
            offset: self.offset + field.offset,
            meta: field.meta,
        }
    }

    pub fn resolve_ptr(&self, parent: *const S) -> *const T {
        unsafe {
            from_raw_parts(
                parent.cast::<u8>()
                    .add(self.offset)
                    .cast::<()>(),
                self.meta,
            )
        }
    }

    pub fn resolve_ptr_mut(&self, parent: *mut S) -> *mut T {
        unsafe {
            from_raw_parts_mut(
                parent.cast::<u8>()
                    .add(self.offset)
                    .cast::<()>(),
                self.meta,
            )
        }
    }

    pub fn resolve_ref<'a>(&self, parent: &'a S) -> &'a T {
        unsafe { &*self.resolve_ptr(parent as *const S) }
    }

    pub fn resolve_mut<'a>(&self, parent: &'a mut S) -> &'a mut T {
        unsafe { &mut *self.resolve_ptr_mut(parent as *mut S) }
    }
}

impl<S: ?Sized, T: ?Sized + Pointee> Copy for Field<S, T> {}
impl<S: ?Sized, T: ?Sized + Pointee> Clone for Field<S, T> {
    fn clone(&self) -> Self {
        Self {
            container_ty: PhantomData,
            offset: self.offset,
            meta: self.meta,
        }
    }
}

// === V-Table === //

type VTableEntries = ConstVec<(RawKey, RawField), { MAX_COMPS }>;

pub struct VTable<S: ?Sized, R: ?Sized> {
    struct_ty: PhantomInvariant<S>,
    root_ty: PhantomInvariant<R>,
    entries: VTableEntries,
}

impl<S, R: ?Sized> VTable<S, R> {
    pub const fn new() -> Self {
        Self {
            struct_ty: PhantomData,
            root_ty: PhantomData,
            entries: ConstVec::new(),
        }
    }

    const fn find_entry(&self, key: RawKey) -> Option<usize> {
        let mut index = 0;
        while index < self.entries.len() {
            let (other_key, _) = self.entries.get(index);
            if key.const_eq(*other_key) {
                return Some (index);
            }
            index += 1;
        }
        None
    }

    pub const unsafe fn expose_raw(&mut self, key: RawKey, field: RawField) {
        let entry = (key, field);
        if let Some (replace_index) = self.find_entry(key) {
            *self.entries.get_mut(replace_index) = entry;
        } else {
            if !self.entries.try_push(entry) {
                // FIXME: Stop hard-coding `MAX_COMPS` in error message once `panic!` supports formatting in `const fn`.
                panic!("VTables can currently only support up to 16 components.");
            }
        }
    }

    pub const fn expose_key<T>(&mut self, key: Key<T>, field: Field<S, T>)
    where
        T: Comp,
        R: RootCastTo<T::Root>,
    {
        unsafe { self.expose_raw(key.raw(), field.as_raw()) };
    }

    // FIXME: Make it work for exposing `dyn Trait` components.
    pub const fn expose_key_unsized<K, T>(&mut self, key: Key<K>, field: Field<S, T>)
    where
        K: ?Sized,
        T: Comp + Unsize<K>,
        R: RootCastTo<T::Root>,
    {
        unsafe { self.expose_raw(key.raw(), field.cast::<K>().as_raw()) };
    }

    pub const fn expose<T>(&mut self, field: Field<S, T>)
    where
        T: 'static + Comp,
        R: RootCastTo<T::Root>,
    {
        self.expose_key(Key::<T>::typed(), field);
    }

    // FIXME: Make it work for exposing `dyn Trait` components.
    pub const fn expose_unsized<K, T>(&mut self, field: Field<S, T>)
    where
        K: ?Sized + 'static,
        T: Comp + Unsize<K>,
        R: RootCastTo<T::Root>,
    {
        self.expose_key_unsized(Key::<K>::typed(), field);
    }

    pub const fn extend<S2>(&mut self, field: Field<S, S2>, other: VTable<S2, R>) {
        let mut index = 0;
        while index < other.entries.len() {
            let (key, subfield) = *other.entries.get(index);
            let new_field = field.as_raw().subfield(&subfield);
            unsafe { self.expose_raw(key, new_field) };

            index += 1;
        }
    }

    pub const fn without(&mut self, key: RawKey) {
        if let Some (index) = self.find_entry(key) {
            self.entries.swap_remove(index);
        }
    }

    pub const fn clone(&self) -> Self {
        Self {
            struct_ty: PhantomData,
            root_ty: PhantomData,
            entries: self.entries.clone(),
        }
    }

    pub const fn build(&self) -> RawVTable {
        RawVTable::new(&self.entries)
    }
}

// We avoid using enums to define the bucket type here, opting instead to write out the optimizations
// manually because rustc doesn't seem to be capable of using their niche layouts to optimize
// pattern matching.  https://godbolt.org/z/93GTWY1P4
#[derive(Copy, Clone)]
struct VTableBucket {
    id: u64,
    field: MaybeUninit<RawField>,
}

impl VTableBucket {
    pub const EMPTY: Self = VTableBucket { id: 0, field: MaybeUninit::uninit() };

    pub const fn full(key: RawKey, field: RawField) -> Self {
        Self {
            id: key.as_u64().get(),
            field: MaybeUninit::new(field),
        }
    }

    pub const fn matches(&self, key: RawKey) -> Option<RawField> {
        if self.id == key.as_u64().get() {
            Some (unsafe { self.field.assume_init() })
        } else {
            None
        }
    }
}

pub struct RawVTable {
    buckets: [VTableBucket; TABLE_CAP],
    mul: u64,
}

impl RawVTable {
    const fn new(entries: &VTableEntries) -> Self {
        // Generate table layout
        #[derive(Copy, Clone)]
        struct VirtualBucket {
            mul: u64,
            entry_idx: usize,
        }

        let mut mul = 0;
        let mut virtual_table = [VirtualBucket { mul, entry_idx: 0 }; TABLE_CAP];

        'gen: loop {
            mul += 1;

            let mut entry_idx = 0;
            while entry_idx < entries.len() {
                let bucket_idx = Self::get_index(entries.get(entry_idx).0.as_u64().get(), mul);
                let bucket = &mut virtual_table[bucket_idx];
                if bucket.mul == mul {
                    continue 'gen;
                }

                bucket.entry_idx = entry_idx;
                bucket.mul = mul;
                entry_idx += 1;
            }

            break;
        }

        // Build table
        let mut buckets = [VTableBucket::EMPTY; TABLE_CAP];
        let mut bucket_idx = 0;
        while bucket_idx < TABLE_CAP {
            let bucket = &virtual_table[bucket_idx];

            if bucket.mul == mul {
                let (key, meta) = *entries.get(bucket.entry_idx);
                buckets[bucket_idx] = VTableBucket::full(key, meta);
            }

            bucket_idx += 1;
        }

        Self { buckets, mul }
    }

    const fn get_index(id: u64, mul: u64) -> usize {
        ((id.wrapping_mul(mul)) % TABLE_CAP as u64) as usize
    }

    pub const fn get(&self, key: RawKey) -> Option<RawField> {
        self.buckets[Self::get_index(key.as_u64().get(), self.mul)]
            .matches(key)
    }
}
