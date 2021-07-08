use std::ops::Deref;
use crate::{key::Key, vtable::RawVTable};
use crate::util::ref_addr;

/// Specifies the root `Obj` type required by the specific component when creating a `CompRef`. All
/// types receive `type Root = dyn Obj` as a default, allowing a `CompRef` to be created with an
/// arbitrary root parameter but this can be overridden.
pub trait Comp {
    type Root: ?Sized;
}

impl<T: ?Sized> Comp for T {
    default type Root = dyn Obj;
}

pub unsafe trait Obj: 'static {
    // TODO: Migrate to associated variable constants once available.
    fn table(&self) -> &'static RawVTable;
}

pub trait DynObjConvert {
    fn to_dyn(&self) -> &dyn Obj;
    unsafe fn from_dyn(obj: &dyn Obj) -> &Self;
}

impl DynObjConvert for dyn Obj {
    fn to_dyn(&self) -> &dyn Obj {
        self
    }

    unsafe fn from_dyn(obj: &dyn Obj) -> &Self {
        obj
    }
}

impl<T: Sized + Obj> DynObjConvert for T {
    fn to_dyn(&self) -> &dyn Obj {
        self
    }

    unsafe fn from_dyn(obj: &dyn Obj) -> &Self {
        &*ref_addr(obj).cast::<T>()
    }
}

pub struct CompRef<'a, T: ?Sized> {
    root: &'a dyn Obj,
    comp: &'a T,
}

impl<'a, T: Comp> CompRef<'a, T> where T::Root: DynObjConvert {
    pub fn new(root: &'a T::Root, comp: &'a T) -> Self {
        Self {
            root: root.to_dyn(),
            comp
        }
    }

    pub fn root(&self) -> &'a T::Root {
        unsafe { T::Root::from_dyn(self.root) }
    }
}

impl<'a, T: ?Sized> CompRef<'a, T> {
    pub unsafe fn new_unsafe(root: &'a dyn Obj, comp: &'a T) -> Self {
        Self { root, comp }
    }

    pub fn root_raw(&self) -> &'a dyn Obj {
        self.root
    }

    pub fn comp(&self) -> &'a T {
        self.comp
    }
}

impl<T: ?Sized + Comp> Deref for CompRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.comp
    }
}

impl<T: ?Sized> Copy for CompRef<'_, T> {}
impl<T: ?Sized> Clone for CompRef<'_, T> {
    fn clone(&self) -> Self {
        Self { root: self.root, comp: self.comp }
    }
}

pub trait ObjExt {
    fn try_fetch_key<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> Option<CompRef<T>>;

    fn fetch_key<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> CompRef<T> {
        self.try_fetch_key(key).unwrap()
    }

    unsafe fn fetch_key_unchecked<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> CompRef<T> {
        self.try_fetch_key(key).unwrap_unchecked()
    }

    fn has_key<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> bool {
        self.try_fetch_key(key).is_some()
    }

    fn try_fetch<T: ?Sized + 'static + Comp<Root = Self>>(&self) -> Option<CompRef<T>> {
        self.try_fetch_key(Key::<T>::typed())
    }

    fn fetch<T: ?Sized + 'static + Comp<Root = Self>>(&self) -> CompRef<T> {
        self.fetch_key(Key::<T>::typed())
    }

    unsafe fn fetch_unchecked<T: ?Sized + 'static + Comp<Root = Self>>(&self) -> CompRef<T> {
        self.fetch_key_unchecked(Key::<T>::typed())
    }

    fn has<T: ?Sized + 'static + Comp<Root = Self>>(&self) -> bool {
        self.has_key(Key::<T>::typed())
    }
}

impl<A: ?Sized + Obj + DynObjConvert> ObjExt for A {
    fn try_fetch_key<'a, T: ?Sized + Comp<Root = Self>>(&'a self, key: Key<T>) -> Option<CompRef<'a, T>> {
        self.table().get(key.raw())
            .map(|entry| unsafe {
                let field: &'a T = entry.fetch_unchecked_ref::<T>(ref_addr(self));
                CompRef::new_unsafe(self.to_dyn(), field)
            })
    }
}
