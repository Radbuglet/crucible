pub trait FuncMethodInjectorRef<T: ?Sized> {
	type Guard<'a>: Deref<Target = T>;
	type Injector;

	const INJECTOR: Self::Injector;
}

pub trait FuncMethodInjectorMut<T: ?Sized> {
	type Guard<'a>: DerefMut<Target = T>;
	type Injector;

	const INJECTOR: Self::Injector;
}

// === `delegate!` macro === //

#[doc(hidden)]
pub mod macro_internal {
	use super::{FuncMethodInjectorMut, FuncMethodInjectorRef};
	use std::ops::DerefMut;

	pub trait FuncMethodInjectorRefGetGuard<T: ?Sized> {
		type GuardHelper<'a>: Deref<Target = T>;
	}

	impl<G, T> FuncMethodInjectorRefGetGuard<T> for G
	where
		T: ?Sized,
		G: FuncMethodInjectorRef<T>,
	{
		type GuardHelper<'a> = G::Guard<'a>;
	}

	pub trait FuncMethodInjectorMutGetGuard<T: ?Sized> {
		type GuardHelper<'a>: DerefMut<Target = T>;
	}

	impl<G, T> FuncMethodInjectorMutGetGuard<T> for G
	where
		T: ?Sized,
		G: FuncMethodInjectorMut<T>,
	{
		type GuardHelper<'a> = G::Guard<'a>;
	}

	pub use std::{
		clone::Clone,
		convert::From,
		fmt,
		marker::{PhantomData, Send, Sync},
		ops::Deref,
		stringify,
		sync::Arc,
	};
}

#[macro_export]
macro_rules! delegate {
	(
		$(#[$attr_meta:meta])*
		$vis:vis fn $name:ident
			$(
				<$($generic:ident),* $(,)?>
				$(<$($fn_lt:lifetime),* $(,)?>)?
			)?
			(
				&$inj_lt:lifetime self [$($inj_name:ident: $inj:ty),* $(,)?]
				$(, $para_name:ident: $para:ty)* $(,)?
			) $(-> $ret:ty)?
		$(where $($where_token:tt)*)?
	) => {
		$crate::delegate! {
			$(#[$attr_meta])*
			$vis fn $name
				< $($($generic),*)? >
				< $inj_lt, $($($($fn_lt),*)?)? >
				(
					$($inj_name: $inj,)*
					$($para_name: $para,)*
				) $(-> $ret)?
			$(where $($where_token)*)?
		}

		impl$(<$($generic),*>)? $name $(<$($generic),*>)?
		$(where
			$($where_token)*
		)? {
			#[allow(unused)]
			pub fn new_method_ref<Injector, Receiver, Func>(_injector: Injector, handler: Func) -> Self
			where
				Injector: 'static + $crate::lang::delegate::macro_internal::FuncMethodInjectorRefGetGuard<Receiver>,
				Injector: $crate::lang::delegate::FuncMethodInjectorRef<
					Receiver,
					Injector = for<
						$inj_lt
						$($(
							$(,$fn_lt)*
						)?)?
					> fn(
						$(&mut $inj),*
					) -> Injector::GuardHelper<$inj_lt>>,
				Receiver: ?Sized + 'static,
				Func: 'static
					+ $crate::lang::delegate::macro_internal::Send
					+ $crate::lang::delegate::macro_internal::Sync
					+ for<$inj_lt $($( $(,$fn_lt)* )?)?> Fn(
						&Receiver,
						$($inj,)*
						$($para,)*
					) $(-> $ret)?,
			{
				Self::new(move |$(mut $inj_name,)* $($para_name,)*| {
					let guard = Injector::INJECTOR($(&mut $inj_name,)*);

					handler(&*guard, $($inj_name,)* $($para_name,)*)
				})
			}

			#[allow(unused)]
			pub fn new_method_mut<Injector, Receiver, Func>(_injector: Injector, handler: Func) -> Self
			where
				Injector: 'static + $crate::lang::delegate::macro_internal::FuncMethodInjectorMutGetGuard<Receiver>,
				Injector: $crate::lang::delegate::FuncMethodInjectorMut<
					Receiver,
					Injector = for<
						$inj_lt
						$($(
							$(,$fn_lt)*
						)?)?
					> fn(
						$(&mut $inj),*
					) -> Injector::GuardHelper<$inj_lt>>,
				Receiver: ?Sized + 'static,
				Func: 'static
					+ $crate::lang::delegate::macro_internal::Send
					+ $crate::lang::delegate::macro_internal::Sync
					+ for<$inj_lt $($( $(,$fn_lt)* )?)?> Fn(
						&mut Receiver,
						$($inj,)*
						$($para,)*
					) $(-> $ret)?,
			{
				Self::new(move |$(mut $inj_name,)* $($para_name,)*| {
					let mut guard = Injector::INJECTOR($(&mut $inj_name,)*);

					handler(&mut *guard, $($inj_name,)* $($para_name,)*)
				})
			}
		}
	};
	(
		$(#[$attr_meta:meta])*
		$vis:vis fn $name:ident
			$(
				<$($generic:ident),* $(,)?>
				$(<$($fn_lt:lifetime),* $(,)?>)?
			)?
			($($para_name:ident: $para:ty),* $(,)?) $(-> $ret:ty)?
		$(where $($where_token:tt)*)?
	) => {
		$(#[$attr_meta])*
		$vis struct $name $(<$($generic),*>)?
		$(where
			$($where_token)*
		)? {
			_ty: ($($($crate::lang::delegate::macro_internal::PhantomData<$generic>,)*)?),
			// TODO: Optimize the internal representation to avoid allocations for context-less handlers.
			handler: $crate::lang::delegate::macro_internal::Arc<
				dyn
					$($(for<$($fn_lt),*>)?)?
					Fn($($para),*) $(-> $ret)? +
						$crate::lang::delegate::macro_internal::Send +
						$crate::lang::delegate::macro_internal::Sync
			>,
		}

		impl$(<$($generic),*>)? $name $(<$($generic),*>)?
		$(where
			$($where_token)*
		)? {
			#[allow(unused)]
			pub fn new<Func>(handler: Func) -> Self
			where
				Func: 'static +
					$crate::lang::delegate::macro_internal::Send +
					$crate::lang::delegate::macro_internal::Sync +
					$($(for<$($fn_lt),*>)?)?
						Fn($($para),*) $(-> $ret)?,
			{
				Self {
					_ty: ($($($crate::lang::delegate::macro_internal::PhantomData::<$generic>,)*)?),
					handler: $crate::lang::delegate::macro_internal::Arc::new(handler),
				}
			}
		}

		impl<
			Func: 'static +
				$crate::lang::delegate::macro_internal::Send +
				$crate::lang::delegate::macro_internal::Sync +
				$($(for<$($fn_lt),*>)?)?
					Fn($($para),*) $(-> $ret)?
			$(, $($generic),*)?
		> $crate::lang::delegate::macro_internal::From<Func> for $name $(<$($generic),*>)?
		$(where
			$($where_token)*
		)? {
			fn from(handler: Func) -> Self {
				Self::new(handler)
			}
		}

		impl$(<$($generic),*>)? $crate::lang::delegate::macro_internal::Deref for $name $(<$($generic),*>)?
		$(where
			$($where_token)*
		)? {
			type Target = dyn $($(for<$($fn_lt),*>)?)? Fn($($para),*) $(-> $ret)? +
				$crate::lang::delegate::macro_internal::Send +
				$crate::lang::delegate::macro_internal::Sync;

			fn deref(&self) -> &Self::Target {
				&*self.handler
			}
		}

		impl$(<$($generic),*>)? $crate::lang::delegate::macro_internal::fmt::Debug for $name $(<$($generic),*>)?
		$(where
			$($where_token)*
		)? {
			fn fmt(&self, fmt: &mut $crate::lang::delegate::macro_internal::fmt::Formatter) -> $crate::lang::delegate::macro_internal::fmt::Result {
				fmt.write_str("delegate::")?;
				fmt.write_str($crate::lang::delegate::macro_internal::stringify!($name))?;
				fmt.write_str("(")?;
				$(
					fmt.write_str($crate::lang::delegate::macro_internal::stringify!($para))?;
				)*
				fmt.write_str(")")?;

				Ok(())
			}
		}

		impl$(<$($generic),*>)? $crate::lang::delegate::macro_internal::Clone for $name $(<$($generic),*>)?
		$(where
			$($where_token)*
		)? {
			fn clone(&self) -> Self {
				Self {
					_ty: ($($($crate::lang::delegate::macro_internal::PhantomData::<$generic>,)*)?),
					handler: $crate::lang::delegate::macro_internal::Clone::clone(&self.handler),
				}
			}
		}
	};
}

use std::ops::{Deref, DerefMut};

pub use delegate;
