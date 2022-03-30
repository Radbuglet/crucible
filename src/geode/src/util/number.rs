use derive_where::derive_where;
use std::error::Error;
use std::fmt::Display;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::marker::PhantomData;
use std::mem::replace;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

// === OptionalUsize === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct OptionalUsize {
	pub raw: usize,
}

impl Default for OptionalUsize {
	fn default() -> Self {
		Self::NONE
	}
}

impl OptionalUsize {
	pub const NONE: Self = Self { raw: usize::MAX };

	pub fn some(value: usize) -> Self {
		debug_assert!(value != usize::MAX);
		Self { raw: value }
	}

	pub fn as_option(self) -> Option<usize> {
		match self {
			OptionalUsize { raw: usize::MAX } => None,
			OptionalUsize { raw: value } => Some(value),
		}
	}
}

// === Number Generation === //

// Traits
pub trait NumberGenBase: Sized {
	type Value: Sized + Debug;

	fn generator_limit() -> Self::Value;
}

pub trait NumberGenRef: NumberGenBase {
	fn try_generate_ref(&self) -> Result<Self::Value, GenOverflowError<Self>>;
}

pub trait NumberGenMut: NumberGenBase {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>>;
}

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct GenOverflowError<D> {
	_ty: PhantomData<D>,
}

impl<D> GenOverflowError<D> {
	pub fn new() -> Self {
		Self::default()
	}
}

impl<D: NumberGenBase> Error for GenOverflowError<D> {}

impl<D: NumberGenBase> Display for GenOverflowError<D> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		writeln!(
			f,
			"generator overflowed (more than {:?} identifiers generated)",
			D::generator_limit(),
		)
	}
}

// Delegation
pub trait NumberGenDelegator {
	type Generator: NumberGenBase;
	type Value: Sized + Debug;

	fn wrap_generated_value(value: <Self::Generator as NumberGenBase>::Value) -> Self::Value;
	fn base_generator(&self) -> &Self::Generator;
	fn base_generator_mut(&mut self) -> &mut Self::Generator;
}

impl<D: NumberGenDelegator<Generator = G>, G: NumberGenBase> NumberGenBase for D {
	type Value = D::Value;

	fn generator_limit() -> Self::Value {
		D::wrap_generated_value(G::generator_limit())
	}
}

impl<D: NumberGenDelegator<Generator = G>, G: NumberGenRef> NumberGenRef for D {
	fn try_generate_ref(&self) -> Result<Self::Value, GenOverflowError<Self>> {
		Ok(D::wrap_generated_value(
			self.base_generator()
				.try_generate_ref()
				.ok()
				.ok_or(GenOverflowError::new())?,
		))
	}
}

impl<D: NumberGenDelegator<Generator = G>, G: NumberGenMut> NumberGenMut for D {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>> {
		Ok(D::wrap_generated_value(
			self.base_generator_mut()
				.try_generate_mut()
				.ok()
				.ok_or(GenOverflowError::new())?,
		))
	}
}

// Primitive generators
impl NumberGenBase for u64 {
	type Value = u64;

	fn generator_limit() -> Self::Value {
		u64::MAX
	}
}

impl NumberGenMut for u64 {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>> {
		Ok(replace(
			self,
			self.checked_add(1).ok_or(GenOverflowError::new())?,
		))
	}
}

impl NumberGenBase for NonZeroU64 {
	type Value = NonZeroU64;

	fn generator_limit() -> Self::Value {
		NonZeroU64::new(u64::MAX).unwrap()
	}
}

impl NumberGenMut for NonZeroU64 {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>> {
		Ok(replace(
			self,
			NonZeroU64::new(self.get().checked_add(1).ok_or(GenOverflowError::new())?).unwrap(),
		))
	}
}

impl NumberGenBase for AtomicU64 {
	type Value = u64;

	fn generator_limit() -> Self::Value {
		u64::MAX - 1000
	}
}

impl NumberGenRef for AtomicU64 {
	fn try_generate_ref(&self) -> Result<Self::Value, GenOverflowError<Self>> {
		let id = self.fetch_add(1, Ordering::Relaxed);

		// Look, unless we manage to allocate more than `1000` IDs before this check runs, this check
		// is *perfectly fine*.
		if id > Self::generator_limit() {
			self.store(Self::generator_limit(), Ordering::Relaxed);
			return Err(GenOverflowError::new());
		}

		Ok(id)
	}
}

impl NumberGenMut for AtomicU64 {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>> {
		if *self.get_mut() >= Self::generator_limit() {
			return Err(GenOverflowError::new());
		} else {
			let next = *self.get_mut() + 1;
			Ok(replace(self.get_mut(), next))
		}
	}
}

#[derive(Debug)]
pub struct NonZeroU64Generator {
	pub counter: AtomicU64,
}

impl Default for NonZeroU64Generator {
	fn default() -> Self {
		Self {
			counter: AtomicU64::new(1),
		}
	}
}

impl NonZeroU64Generator {
	pub fn next_value(&mut self) -> NonZeroU64 {
		NonZeroU64::new(*self.counter.get_mut()).unwrap()
	}
}

impl NumberGenBase for NonZeroU64Generator {
	type Value = NonZeroU64;

	fn generator_limit() -> Self::Value {
		NonZeroU64::new(AtomicU64::generator_limit()).unwrap()
	}
}

impl NumberGenRef for NonZeroU64Generator {
	fn try_generate_ref(&self) -> Result<Self::Value, GenOverflowError<Self>> {
		let id = self
			.counter
			.try_generate_ref()
			.ok()
			.ok_or(GenOverflowError::new())?;

		Ok(NonZeroU64::new(id).unwrap())
	}
}

impl NumberGenMut for NonZeroU64Generator {
	fn try_generate_mut(&mut self) -> Result<Self::Value, GenOverflowError<Self>> {
		let id = self
			.counter
			.try_generate_mut()
			.ok()
			.ok_or(GenOverflowError::new())?;

		Ok(NonZeroU64::new(id).unwrap())
	}
}

// === usize bit-masking === //

pub const fn u64_msb_mask(offset: u32) -> u64 {
	debug_assert!(offset < 64);
	1u64.rotate_right(offset + 1)
}

pub const fn u64_has_mask(value: u64, mask: u64) -> bool {
	value | mask == value
}
