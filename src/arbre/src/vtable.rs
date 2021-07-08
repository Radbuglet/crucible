use std::ptr::{DynMetadata, Pointee, from_raw_parts};
use crate::fetch::Comp;
use crate::key::{RawKey, Key};
use crate::util::{AnyValue, ConstVec, PerfectMap, PhantomInvariant, ref_addr};

const TABLE_BUCKETS: usize = 32;
const MAX_COMPS: usize = 16;

pub struct Field<S, T: ?Sized> {
    _ty: PhantomInvariant<(S, T)>,
    raw: RawField,
}

pub macro new_field() {
    // TODO
}

impl<T> Field<T, T> {
    pub const fn identity() -> Self {
        unsafe {
            Self::from_raw(RawField::new(0, ()))
        }
    }
}

impl<S, V: ?Sized + Pointee> Field<S, V> {
    pub const unsafe fn new_unchecked(offset: usize, meta: V::Metadata) -> Self {
        Self::from_raw(RawField::new(offset, meta))
    }
}

impl<S, V: ?Sized> Field<S, V> {
    pub const unsafe fn from_raw(raw: RawField) -> Self {
        Self {
            _ty: PhantomInvariant::new(),
            raw,
        }
    }

    pub const fn raw(&self) -> RawField {
        self.raw
    }

    pub fn fetch(&self, root: &S) -> *const V {
        unsafe { self.raw.fetch_unchecked::<V>(ref_addr(root)) }
    }
}

#[derive(Copy, Clone)]
pub struct RawField {
    /// The offset to the actual object's address.
    offset: usize,

    /// The pointer's metadata (unit if the pointer is thin).
    // All `dyn` metadata currently has the same size and layout. Let's hope it stays that way.
    // Technically, `AnyValue` can store all current forms of metadata generated by the compiler but
    // I wouldn't rely on that remaining true.
    meta: AnyValue<DynMetadata<()>>,
}

impl RawField {
    pub const fn new<T>(offset: usize, meta: T) -> Self {
        Self { offset, meta: AnyValue::new(meta) }
    }

    pub unsafe fn fetch_unchecked<T: ?Sized>(&self, root: *const ()) -> *const T {
        let addr = root.add(self.offset);
        let meta = self.meta.get::<<T as Pointee>::Metadata>();
        from_raw_parts::<T>(addr, meta)
    }

    pub unsafe fn fetch_unchecked_ref<T: ?Sized>(&self, root: *const ()) -> &T {
        &*self.fetch_unchecked(root)
    }
}

// === Raw V-Table === //

pub struct RawVTable {
    map: PerfectMap<RawField, { TABLE_BUCKETS }>,
}

impl RawVTable {
    pub(crate) fn get(&self, key: RawKey) -> Option<&RawField> {
        self.map.get(key.map_key())
    }
}

// === Typed V-Table === //

pub struct VTable<S, R: ?Sized> {
    _ty: PhantomInvariant<(S, R)>,
    entries: ConstVec<(RawKey, RawField), { MAX_COMPS }>,
}

impl<S, R: ?Sized> VTable<S, R> {
    pub const fn new() -> Self {
        Self {
            _ty: PhantomInvariant::new(),
            entries: ConstVec::new(),
        }
    }

    const fn get_inner(&self, key: RawKey) -> Option<(usize, &RawField)> {
        let mut index = 0;
        while index < self.entries.len() {
            let (other_key, entry) = self.entries.get(index);
            if key.eq_other(*other_key) {
                return Some ((index, entry));
            }
            index += 1;
        }
        None
    }

    pub const fn expose_key<T: ?Sized + Comp<Root = R>>(&mut self, key: Key<T>, field: Field<S, T>) {
        todo!()
    }

    pub const fn expose<T: ?Sized + 'static + Comp<Root = R>>(&mut self, field: Field<S, T>) {
        self.expose_key(Key::<T>::typed(), field);
    }

    pub const fn without(&mut self, key: RawKey) {
        todo!()
    }

    pub const fn merge(&mut self, other: Self) {
        todo!()
    }

    pub const fn expand(&mut self, other: Self) {
        todo!()
    }

    pub const fn extend<S2>(&mut self, field: Field<S, S2>, other: VTable<S2, R>) {
        todo!()
    }

    pub const fn clone(&self) -> Self {
        todo!()
    }

    pub const fn build(&self) -> RawVTable {
        todo!()
    }
}
