use std::ops::Deref;
use crate::key::Key;
use crate::vtable::{VTable, RawVTable};
use crate::util::ref_addr;

// === Component === //

/// Specifies the root [Obj] type required by the specific component when creating a [CompRef]. All
/// types receive `type Root = dyn Obj` as a default, allowing a [CompRef] to be created with an
/// arbitrary root parameter but this can be overridden by implementing this trait manually.
pub trait Comp {
    type Root: ?Sized;
}

impl<T: ?Sized> Comp for T {
    default type Root = dyn Obj;
}

/// An internal trait allowing a type to be converted from `Self` to `dyn Obj` and back. All `Sized`
/// types implementing `Obj` can be converted back and forth into `dyn Obj` but only the `dyn Obj`
/// unsized type can be converted to the `dyn Obj` representation. This is because of limitations on
/// trait conversion: `&dyn Trait` references only contain the v-table of .
#[doc(hidden)]
pub trait DynObjConvert {
    /// Converts the object to `dyn Obj`.
    fn to_dyn(&self) -> &dyn Obj;

    /// Converts a `dyn Obj` instance back to its original type.
    ///
    /// It is perfectly acceptable to use this method to convert the `dyn Obj` returned from another
    /// converter. For example, [ObjExt] may potentially use the no-op `dyn Obj` to `dyn Obj`
    /// converter to fill in the root field of the [CompRef] instance. This [CompRef], once being
    /// resolved from a `CompRef<dyn MyTrait>` to a `CompRef<MyConcreteType>` through dynamic dispatch,
    /// may be converted to the concrete root type using the `<MyConcreteType as Comp>::Root`
    /// implementation of [DynObjConvert::from_dyn].
    ///
    /// ## Safety
    ///
    /// The target type must either be a `dyn Obj` or the underlying pointee type of the `dyn Obj`
    /// reference.
    ///
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
        // Safety: provided by caller
        &*ref_addr(obj).cast::<T>()
    }
}

/// A reference to a component within an [Obj] that also included the [Obj] instance from which it was
/// derived. By requiring specific root object types through the [Comp::Root] associated type,
/// the component can derive the original type of [Obj].
///
/// [CompRef] provides no guarantee that the referenced `root` is actually the top-level root of the
/// object and, in fact, provides a safe mechanism for users to bundle their own root with a component.
/// This may impact the soundness of certain [ObjExt::fetch_key_unchecked] operations.
pub struct CompRef<'a, T: ?Sized> {
    root: &'a dyn Obj,
    comp: &'a T,
}

impl<'a, T: Comp> CompRef<'a, T> where T::Root: DynObjConvert {
    /// Constructs a new [CompRef] with an (appropriately typed) object root. The type of the root is
    /// determined by the type of the component's [Comp::Root] associated type, which is `dyn Obj` by
    /// default.
    ///
    /// This method only works on `Sized` or `dyn Obj` [Comp::Root] types. The type of the component
    /// must be `Sized` since [Comp] is not object safe.
    pub fn new(root: &'a T::Root, comp: &'a T) -> Self {
        Self {
            root: root.to_dyn(),
            comp
        }
    }

    /// Fetches the [Obj] root under the type requested by the component's [Comp] implementation,
    /// which is `dyn Obj` by default. When `Comp::Root` is `Sized`, this reference should be
    /// functionally equivalent to the reference returned by [root_raw], but the additional type
    /// information gained by obtaining the object's concrete type (which is a zero-cost operation)
    /// will allow the compiler to elide fetches and dynamic dispatches when the component is `dyn`,
    /// greatly improving performance.
    ///
    /// The lifetime of the returned reference is the lifetime of the [CompRef] instance and not the
    /// lifetime of any references to it, potentially allowing the reference to the root to outlive
    /// the lifetime of [CompRef]'s borrow. Since [CompRef] is [Copy], instances are generally passed
    /// by value instead of by reference.
    ///
    /// This method only works on `Sized` or `dyn Obj` [Comp::Root] types. The type of the component
    /// must be `Sized` since [Comp] is not object safe. If these requirements cannot be met, [root_raw]
    /// may be used instead.
    pub fn root(&self) -> &'a T::Root {
        // Safety: this object's invariants require that `root` point to an instance of `T::Root`.
        unsafe { T::Root::from_dyn(self.root) }
    }
}

impl<'a, T: ?Sized> CompRef<'a, T> {
    /// Constructs a new [CompRef] with an (appropriately typed) object root. In cases where `T: Sized`,
    /// you almost certainly want to use the safe variant [new], which checks the validity of the `root`
    /// type at compile time. However, if you're creating a reference to a component that is `?Sized`,
    /// [new_unsafe] is the only option.
    ///
    /// ## Safety
    ///
    /// [CompRef] maintains that `root`'s ype must always match `comp's` [Comp::Root] type.
    ///
    /// While there is no way for users to run [CompRef::root] when `T: ?Sized` (the only method
    /// relying on this invariant), users can dynamically invoke component methods which automatically
    /// and safely coerce `dyn SomeTrait` to the component's concrete (`Sized`) type.
    ///
    pub unsafe fn new_unsafe(root: &'a dyn Obj, comp: &'a T) -> Self {
        Self { root, comp }
    }

    /// Fetches the root under the generic `dyn Obj` type. Fetches through `dyn Obj` are generally
    /// slower than fetches through the root's concrete (`Sized`) type so it is advised to use [root]
    /// where possible.
    ///
    /// The lifetime of the returned reference is the lifetime of the [CompRef] instance and not the
    /// lifetime of any references to it, potentially allowing the reference to the root to outlive
    /// the lifetime of [CompRef]'s borrow. Since [CompRef] is [Copy], instances are generally passed
    /// by value instead of by reference.
    pub fn root_raw(&self) -> &'a dyn Obj {
        self.root
    }

    /// Fetches a component reference. Unlike the component references returned by [Deref], the
    /// lifetime of the returned reference is the lifetime of the [CompRef] instance and not the
    /// lifetime of any references to that instance. This means that references returned by this
    /// method may outlive references returned by [Deref]. Since [CompRef] is [Copy], instances are
    /// generally passed by value instead of by references, making both methods equivalent.
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

// === Object === //

/// A utility trait to safely derive [Obj] implementations. [ObjDecl] is not object-safe and `dyn Obj`
/// must be used to pass object references instead.
///
/// [ObjDecl] has two parts: `Root` and `TABLE`.
///
/// `Root` specifies the type of the root object from which components are fetched. Every component
/// inside an object's V-Table must share the same or a lesser requirement for the type of `Root` as
/// specified in their implementation of [Comp]. A `Root` type of `dyn Obj` in both [ObjDecl] and
/// [Comp] means that the components can be fetched under any root. [Obj] is only derived on instances
/// which are valid as their own root.
///
/// `TABLE` is a v-table mapping [Keys] to their pointer offset within the struct. See [VTable] for
/// more details.
pub trait ObjDecl: Sized {
    type Root: ?Sized;
    const TABLE: VTable<Self, Self::Root>;
}

// TODO: Fix signature (`Root = dyn Obj` should work as well).
unsafe impl<T: 'static + ObjDecl<Root = T>> Obj for T {
    fn table(&self) -> &'static RawVTable {
        todo!()
    }
}


/// An object-safe trait allowing the object to be queried for components using the [ObjExt] extension
/// methods. This trait can be derived safely using the [ObjDecl] declarative trait. Out of the three
/// aforementioned traits, `dyn Obj` is likely the trait you want to use when passing an object
/// instance.
///
/// `Obj` is currently limited to a `'static` lifetime to simplify its implementation and soundness
/// analysis. This requirement may be lifted in the future.
///
/// ## Safety
///
/// TODO
///
// TODO: Consider allowing non-`'static` objects in the future.
pub unsafe trait Obj: 'static {
    // TODO: Migrate to associated variable constants once available.
    fn table(&self) -> &'static RawVTable;
}

// TODO: Document
pub trait ObjExt {
    fn try_fetch_key<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> Option<CompRef<T>>;

    fn fetch_key<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> CompRef<T> {
        self.try_fetch_key(key).unwrap()
    }

    unsafe fn fetch_key_unchecked<T: ?Sized + Comp<Root = Self>>(&self, key: Key<T>) -> CompRef<T> {
        // Safety: provided by caller
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
        // Safety: provided by caller
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
                // TODO: Safety
                let field: &'a T = entry.fetch_unchecked_ref::<T>(ref_addr(self));
                CompRef::new_unsafe(self.to_dyn(), field)
            })
    }
}
