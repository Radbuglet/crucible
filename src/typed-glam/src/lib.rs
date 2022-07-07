use core::{
	fmt, hash,
	iter::{Product, Sum},
	marker::PhantomData,
	mem::transmute,
};

// === Modules === //

#[path = "generated/op_forwards.rs"]
mod op_forwards;

// === Trait definitions === //

pub(crate) mod backing_vec {
	pub trait Sealed {}
}

pub trait BackingVec:
    // This bound ensures that users cannot extend this trait.
    backing_vec::Sealed +
    // These bounds encode some of the properties common to all backing vectors. The remaining
    // "properties" are derived by snippets of code generated for every vector type.
    Sized + fmt::Debug + fmt::Display + Copy + PartialEq + Default
{
}

pub trait VecFlavor {
	type Backing: BackingVec;
}

// === `TypedVector` === //

pub type TypedVector<F> = TypedVectorImpl<<F as VecFlavor>::Backing, F>;

// We keep `B` as its own parameter—despite it being trivially re-derivable—so Rust can figure out
// the difference between an `impl` on all flavors that have a `BackingVec = IVec3` and an `impl` on
// all flavors that have a `BackingVec = UVec3`. For most intents and purposes, we can just use
// `NewTypeVector` directly. It is, after all, the only valid choice for generic parameter pairs.
#[repr(transparent)]
pub struct TypedVectorImpl<B: BackingVec, F: ?Sized + VecFlavor<Backing = B>> {
	_flavor: PhantomData<fn(F) -> F>,
	vec: B,
}

impl<F: ?Sized + VecFlavor> TypedVector<F> {
	pub const fn from_raw(vec: F::Backing) -> Self {
		Self {
			_flavor: PhantomData,
			vec,
		}
	}

	pub const fn from_raw_ref(vec: &F::Backing) -> &Self {
		unsafe {
			// Safety: `NewTypeVectorImpl` is `repr(transparent)` w.r.t `F::Backing` and thus so is
			// its reference.
			transmute(vec)
		}
	}

	pub fn from_raw_mut(vec: &mut F::Backing) -> &mut Self {
		unsafe {
			// Safety: `NewTypeVectorImpl` is `repr(transparent)` w.r.t `F::Backing` and thus so is
			// its reference.
			transmute(vec)
		}
	}

	pub const fn into_raw(self) -> F::Backing {
		self.vec
	}

	pub const fn raw(&self) -> &F::Backing {
		&self.vec
	}

	pub fn raw_mut(&mut self) -> &mut F::Backing {
		&mut self.vec
	}

	pub const fn cast<OF>(self) -> TypedVector<OF>
	where
		OF: VecFlavor<Backing = F::Backing>,
	{
		TypedVector::from_raw(self.into_raw())
	}

	pub const fn cast_ref<OF>(&self) -> &TypedVector<OF>
	where
		OF: VecFlavor<Backing = F::Backing>,
	{
		TypedVector::from_raw_ref(self.raw())
	}

	pub fn cast_mut<OF>(&mut self) -> &mut TypedVector<OF>
	where
		OF: VecFlavor<Backing = F::Backing>,
	{
		TypedVector::from_raw_mut(self.raw_mut())
	}

	pub(crate) fn map_raw<C>(self, f: C) -> Self
	where
		C: FnOnce(F::Backing) -> F::Backing,
	{
		Self::from_raw(f(self.into_raw()))
	}
}

// Basic `impl`s

impl<F: ?Sized + VecFlavor> fmt::Debug for TypedVector<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(self.raw(), f)
	}
}

impl<F: ?Sized + VecFlavor> fmt::Display for TypedVector<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Display::fmt(self.raw(), f)
	}
}

impl<F: ?Sized + VecFlavor> Copy for TypedVector<F> {}

impl<F: ?Sized + VecFlavor> Clone for TypedVector<F> {
	fn clone(&self) -> Self {
		Self {
			_flavor: self._flavor,
			vec: self.vec,
		}
	}
}

impl<F: ?Sized + VecFlavor> PartialEq for TypedVector<F> {
	fn eq(&self, other: &Self) -> bool {
		self.vec == other.vec
	}
}

impl<F: ?Sized + VecFlavor> Eq for TypedVector<F> where F::Backing: Eq {}

impl<F: ?Sized + VecFlavor> hash::Hash for TypedVector<F>
where
	F::Backing: hash::Hash,
{
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.vec.hash(state);
	}
}

impl<'a, F: ?Sized + VecFlavor> Sum<&'a TypedVector<F>> for TypedVector<F>
where
	F: VecFlavor,
	F::Backing: 'a + Sum<&'a F::Backing>,
{
	fn sum<I: Iterator<Item = &'a TypedVector<F>>>(iter: I) -> Self {
		Self::from_raw(iter.map(|v| v.raw()).sum())
	}
}

impl<'a, F: ?Sized + VecFlavor> Product<&'a TypedVector<F>> for TypedVector<F>
where
	F: VecFlavor,
	F::Backing: 'a + Product<&'a F::Backing>,
{
	fn product<I: Iterator<Item = &'a TypedVector<F>>>(iter: I) -> Self {
		Self::from_raw(iter.map(|v| v.raw()).product())
	}
}