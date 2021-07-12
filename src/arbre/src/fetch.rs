use std::fmt;
use std::ops::Deref;
use crate::key::Key;
use crate::vtable::{VTable, RawVTable};
use crate::util::ref_addr;

// === Component === //

/// Specifies the root [Obj] type required by the specific component when creating a [CompRef]. A
/// value of `dyn Obj` allows a [CompRef] to be created with any root parameter implementing `Obj`.
/// Every `?Sized` type has an implementation where `Root = dyn Obj` but this can be specialized by
/// implementing the trait manually.
pub trait Comp {
    type Root: ?Sized;
}

// FIXME: Add blanket impl that doesn't run into opaque associated type issues.
// impl<T: ?Sized> Comp for T {
//     default type Root = dyn Obj;
// }

/// An _**internal**_ trait which performs each individual cast in the component root casting model. In
/// this model, the root from which a component is fetched is upcasted to a `dyn Obj` and then later
/// down-casted to its concrete form once [CompRef]'s full type is derived. The [RootCastTo] trait
/// compliments this trait by defining whether such a round-trip is possible.
///
/// All `Sized` types implementing `Obj` can be converted back and forth into `dyn Obj` but only the
/// `dyn Obj` unsized type can be converted to the `dyn Obj` representation. This is because of
/// limitations on  trait conversion: `&dyn Trait` references only contain `Trait`'s v-table, with
/// no way to downcast it to other traits.
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

/// An _**internal**_ trait which indicates that a certain type can be legally casted from `Self` to
/// `Target` through [DynObjConvert]'s `Self -> dyn Obj -> Target` casting scheme.
///
/// ## Safety
///
/// See above definition. This trait must always remain consistent with [DynObjConvert]'s actual
/// capabilities *i.e.* a `dyn Obj` obtained from `Self` must be safely castable to `Target` through
/// [from_dyn].
///
pub unsafe trait RootCastTo<Target: ?Sized> {}

unsafe impl<T: ?Sized> RootCastTo<T> for T {}  // Reflexive static casts
unsafe impl<T: Sized> RootCastTo<dyn Obj> for T {}  // Weakening casts

/// A reference to a component within an [Obj] which includes the root [Obj] instance from which it
/// was derived. The concrete type of the root is specified by the component's [Comp::Root] associated
/// type parameter.
///
/// [CompRef] provides no guarantee that the referenced root is actually the top-level root of the
/// object and, in fact, provides a safe mechanism for users to bundle their own root with a component.
/// This may impact the soundness of certain [ObjExt::fetch_key_unchecked] operations.
pub struct CompRef<'a, T: ?Sized> {
    root: &'a dyn Obj,
    comp: &'a T,
}

impl<'a, T: Comp> CompRef<'a, T> {
    /// Constructs a new [CompRef] with an appropriately typed object root. The type of the root is
    /// determined by the type of the component's [Comp::Root] associated type, which is `dyn Obj` by
    /// default.
    ///
    /// This method only works on `Sized` or `dyn Obj` [Comp::Root] types. The type of the component
    /// must be `Sized` since [Comp] is not object safe.
    pub fn new<R: ?Sized>(root: &'a R, comp: &'a T) -> Self
    where
        R: RootCastTo<T::Root> + DynObjConvert
    {
        Self {
            root: root.to_dyn(),
            comp
        }
    }
}

impl<'a, T: Comp> CompRef<'a, T> where T::Root: DynObjConvert {
    /// Fetches the [Obj] root under the type requested by the component's [Comp] implementation,
    /// which is `dyn Obj` by default. When `Comp::Root` is `Sized`, this reference is functionally
    /// equivalent to the reference returned by [root_raw], but the additional type information gained
    /// by obtaining the object's concrete type (which is a zero-cost operation) will allow the compiler
    /// to elide fetches and dynamic dispatches when the component is `dyn`, greatly improving
    /// performance.
    ///
    /// The lifetime of the returned reference is the lifetime of the [CompRef] instance and not the
    /// lifetime of any references to it, potentially allowing the reference to the root to outlive
    /// the lifetime of [CompRef]'s borrow. Since [CompRef] is [Copy], instances are generally passed
    /// by value instead of by reference.
    ///
    /// This method only works when [Comp::Root] is `Sized` or `dyn Obj`. The type of the component
    /// `T` must be `Sized` since [Comp] is not object safe. If these requirements cannot be met,
    /// [root_raw] may be used instead.
    pub fn root(&self) -> &'a T::Root {
        // Safety: this object's invariants require that `root` point to an instance of `T::Root`.
        unsafe { T::Root::from_dyn(self.root) }
    }
}

impl<'a, T: ?Sized> CompRef<'a, T> {
    /// Constructs a new [CompRef] with an (appropriately typed) object root. In cases where `T: Sized`,
    /// you almost certainly want to use the safe variant [new], which checks the validity of the `root`
    /// type at compile time. However, if you're creating a reference to a component that is `?Sized`,
    /// this method is your only option.
    ///
    /// ## Safety
    ///
    /// [CompRef] maintains that `root`'s type must be castable to `comp's` [Comp::Root] type. Cast
    /// validity is entirely dictated by an implementation of [RootCastTo].
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

impl<T: ?Sized + fmt::Debug> fmt::Debug for CompRef<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.comp.fmt(f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for CompRef<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.comp.fmt(f)
    }
}

// === Object === //

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
/// 1: Every field in the v-table must always be valid for the struct. In essence, this means that
///
/// - Every entry's offset must point to a field whose type corresponds with the entry's [Key] type,
///   including its lifetime requirements. Metadata must be valid for the reference.
/// - Each entry's field offset must result in a properly aligned reference for all properly aligned
///   instances of the struct. This means that both the field offset and the struct's overall alignment
///   must be checked.
/// - These must *always* hold true for the struct. This means that offsets to union and enum variants
///   will likely cause unsoundness.
///
/// 2: Furthermore, each field must be capable of casting `Self` to the desired root parameter. Cast
/// validity is entirely dictated by the [RootCastTo] trait.
///
/// All these requirements should hold for all [RawVTables] built from their properly typed [VTable]
/// counterpart, hence the equally powerful safe [ObjDecl] utility trait.
// TODO: Consider allowing non-`'static` objects in the future.
pub unsafe trait Obj: 'static {
    // TODO: Migrate to associated variable constants once available.
    fn table(&self) -> &RawVTable;
}

/// A utility trait to safely derive [Obj] implementations. [ObjDecl] is not object-safe so `dyn Obj`
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

impl<T: ObjDecl> Comp for T {
    type Root = <T as ObjDecl>::Root;
}

/// An _**internal**_ trait used to derive [Obj] for all types implementing [ObjDecl].
#[doc(hidden)]
pub unsafe trait ObjConst: 'static {
    const RAW_TABLE: RawVTable;
}

unsafe impl<T: 'static + ObjDecl + RootCastTo<T::Root>> ObjConst for T {
    const RAW_TABLE: RawVTable = T::TABLE.build();
}

unsafe impl<T: ObjConst> Obj for T {
    fn table(&self) -> &RawVTable {
        &Self::RAW_TABLE
    }
}

/// An extension trait for [Obj] providing querying methods. Users are free to derive this trait on
/// other "fetch targets" (e.g. a helper which fetches components from some node hierarchy) and this
/// trait only requires an implementation of [ObjExt::try_fetch_key].
pub trait ObjExt {
    /// Fetches a component based off the passed [Key], returning `None` if the component isn't present.
    ///
    /// This is the only required trait for user-defined implementations of [ObjExt].
    ///
    /// See also: [try_fetch], which fetches a component from its singleton key.
    fn try_fetch_key<T: ?Sized>(&self, key: Key<T>) -> Option<CompRef<T>>;

    /// Fetches a component based off the passed [Key], panicking if the component isn't present.
    ///
    /// See also: [fetch], which fetches a component from its singleton key.
    fn fetch_key<T: ?Sized>(&self, key: Key<T>) -> CompRef<T> {
        self.try_fetch_key(key).unwrap()
    }

    /// Fetches a component based off the passed [Key], causing UB if the component isn't present.
    /// This method is likely unnecessary if `Obj`'s implementation is statically known since the
    /// compiler should be able to elide the fetch entirely.
    ///
    /// See also: [fetch_unchecked], which fetches a component from its singleton key.
    ///
    /// ## Safety
    ///
    /// The caller must guarantee that the component is present. This guarantee may be difficult to
    /// prove, especially when interacting with [CompRef]. Be sure to check what each object's *formal*
    /// guarantees before using this method (in particular, [CompRef] may not always be created under
    /// the same root instance).
    ///
    /// This method only really serves as a slightly more efficient version of [fetch_key], as the
    /// unreachable hint can allow the compiler to skip over the `is_none` check in `unwrap`. If
    /// these requirements are too hard to meet, [fetch_key] can be a much safer direct substitute.
    unsafe fn fetch_key_unchecked<T: ?Sized>(&self, key: Key<T>) -> CompRef<T> {
        // Safety: provided by caller
        self.try_fetch_key(key).unwrap_unchecked()
    }

    /// Returns whether the object has a component under the passed [Key].
    ///
    /// See also: [has], which looks for a component under its singleton key.
    fn has_key<T: ?Sized>(&self, key: Key<T>) -> bool {
        self.try_fetch_key(key).is_some()
    }

    /// Fetches a component under the singleton key obtained from the component type (`T`) itself,
    /// returning `None` if the component isn't present.
    ///
    /// See also: [try_fetch_key], which fetches a component from an arbitrary user-supplied [Key].
    fn try_fetch<T: ?Sized + 'static>(&self) -> Option<CompRef<T>> {
        self.try_fetch_key(Key::<T>::typed())
    }

    /// Fetches a component under the singleton key obtained from the component type (`T`) itself,
    /// panicking if the component isn't present.
    ///
    /// See also: [fetch_key], which fetches a component from an arbitrary user-supplied [Key].
    fn fetch<T: ?Sized + 'static>(&self) -> CompRef<T> {
        self.fetch_key(Key::<T>::typed())
    }

    /// Fetches a component under the singleton key obtained from the component type (`T`) itself, causing
    /// UB if the component isn't present. This method is likely unnecessary if `Obj`'s implementation
    /// is statically known since the compiler should be able to elide the fetch entirely.
    ///
    /// See also: [fetch_key_unchecked], which fetches a component from an arbitrary user-supplied [Key].
    ///
    /// ## Safety
    ///
    /// See the safety section in [fetch_key_unchecked].
    ///
    unsafe fn fetch_unchecked<T: ?Sized + 'static>(&self) -> CompRef<T> {
        // Safety: provided by caller
        self.fetch_key_unchecked(Key::<T>::typed())
    }

    /// Returns whether the object has a component associated with the singleton key obtained from
    /// the component type (`T`) itself.
    ///
    /// See also: [has_ley], which looks for a component under an arbitrary user-supplied [Key].
    fn has<T: ?Sized + 'static>(&self) -> bool {
        self.has_key(Key::<T>::typed())
    }
}

impl<A: ?Sized + Obj + DynObjConvert> ObjExt for A {
    fn try_fetch_key<'a, T: ?Sized>(&'a self, key: Key<T>) -> Option<CompRef<'a, T>> {
        self.table().get(key.raw())
            .map(|entry| unsafe {
                // Safety: We know from v-table's first invariant that this entry references a component
                // of type `T` and that it can be borrowed for the lifetime of the struct.
                let field: &'a T = &*(entry.typed::<Self, T>().resolve_ref(self));

                // Safety: We know from v-table's second invariant that the `dyn Obj` representation
                // of the root can be down-casted into the requested root type.
                CompRef::new_unsafe(self.to_dyn(), field)
            })
    }
}
