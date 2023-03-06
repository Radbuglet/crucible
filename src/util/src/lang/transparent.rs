#[macro_export]
macro_rules! transparent {
	($(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident
			$(<
				$($lt:lifetime),*
				$(,)?
				$($para:ident),*
				$(,)?
			>)?
			(pub $raw:ty$(, $dummy:ty)?)
			$( where { $($where_clause:tt)* } )?;
	)*) => {$(
		$crate::transparent! {
			$(#[$attr])*
			$vis struct $name
				$(<
					$($lt,)*
					$($para,)*
				>)?
				(pub(crate) $raw$(, $dummy)?)
				$( where { $($where_clause)* } )?;
		}

		impl<$($($lt,)* $($para,)*)?> $crate::lang::transparent::macro_internal::From<$raw> for $name<$($($lt,)* $($para,)*)?>
		$(where $($where_clause)*)?
		{
			fn from(raw: $raw) -> Self {
				Self::wrap(raw)
			}
		}

		impl<$($($lt,)* $($para,)*)?> $crate::lang::transparent::macro_internal::From<$name<$($($lt,)* $($para,)*)?>> for $raw
		$(where $($where_clause)*)?
		{
			fn from(me: $name<$($($lt,)* $($para,)*)?>) -> $raw {
				me.raw
			}
		}
	)*};
	// Base implementation without conversion traits
	($(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident
			$(<
				$($lt:lifetime),*
				$(,)?
				$($para:ident),*
				$(,)?
			>)?
			($rvis:vis $raw:ty$(, $dummy:ty)?)
			$( where { $($where_clause:tt)* } )?;
	)*) => {$(
		$(#[$attr])*
		#[repr(transparent)]
		$vis struct $name<$($($lt,)* $($para,)*)?>
		$(where $($where_clause)*)?
		{
			$($rvis ty: $crate::lang::transparent::macro_internal::PhantomData<$dummy>,)?
			$rvis raw: $raw,
		}

		impl<$($($lt,)* $($para,)*)?> $name<$($($lt,)* $($para,)*)?>
		$(where $($where_clause)*)?
		{
			$rvis const fn wrap(raw: $raw) -> Self
			where
				$crate::lang::transparent::macro_internal::SizedHack<$raw, Self>: Sized,
			{
				Self {
					$(ty: $crate::lang::transparent::macro_internal::PhantomData::<$dummy>,)?
					raw,
				}
			}

			$rvis fn wrap_ref<'_rlt>(raw: &'_rlt $raw) -> &'_rlt Self {
				unsafe { $crate::lang::transparent::macro_internal::transmute(raw) }
			}

			$rvis fn wrap_mut<'_rlt>(raw: &'_rlt mut $raw) -> &'_rlt mut Self {
				unsafe { $crate::lang::transparent::macro_internal::transmute(raw) }
			}
		}
	)*};
}

pub use transparent;

#[doc(hidden)]
pub mod macro_internal {
	pub use core::{convert::From, marker::PhantomData, mem::transmute};

	pub struct SizedHack<A: ?Sized, B: ?Sized>(PhantomData<A>, PhantomData<B>);

	pub trait IsSized {}

	impl<A, B> IsSized for SizedHack<A, B> {}
}
