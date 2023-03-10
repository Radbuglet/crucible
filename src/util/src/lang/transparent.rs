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
				(pub $raw$(, $dummy)?)
				$( where { $($where_clause)* } )?;

			;FORCE_NO_TRAIT
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
	)* $(;FORCE_NO_TRAIT)?) => {$(
		$(#[$attr])*
		#[repr(transparent)]
		$vis struct $name<$($($lt,)* $($para,)*)?>
		$(where $($where_clause)*)?
		{
			$($rvis ty: $crate::lang::transparent::macro_internal::PhantomData<$dummy>,)?
			$rvis raw: $raw,
		}

		#[allow(dead_code)]
		impl<$($($lt,)* $($para,)*)?> $name<$($($lt,)* $($para,)*)?>
		$(where $($where_clause)*)?
		{
			$rvis const fn wrap(raw: $raw) -> Self
			where
				for<'trivial> <$raw as $crate::lang::transparent::macro_internal::TrivialBounds<'trivial>>::Of: Sized,
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

	pub trait TrivialBounds<'a> {
		type Of: ?Sized;
	}

	impl<'a, T: ?Sized> TrivialBounds<'a> for T {
		type Of = T;
	}
}
