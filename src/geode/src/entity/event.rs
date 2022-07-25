use std::{
	cell::{RefCell, RefMut},
	ops::{Deref, DerefMut},
};

use bytemuck::TransparentWrapper;

use super::entity::Entity;
use crate::core::session::Session;

// === Delegate Core === //

pub trait PromoteMut<'a> {
	type Value: ?Sized;
	type Guard: DerefMut<Target = Self::Value>;

	fn promote_mut(&'a self) -> Self::Guard;
}

impl<'a, T: ?Sized + 'a> PromoteMut<'a> for RefCell<T> {
	type Value = T;
	type Guard = RefMut<'a, Self::Value>;

	fn promote_mut(&'a self) -> Self::Guard {
		self.borrow_mut()
	}
}

#[derive(Debug, TransparentWrapper)]
#[repr(transparent)]
pub struct DelegateAutoBorrow<T: ?Sized>(pub T);

impl<T: ?Sized> Deref for DelegateAutoBorrow<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: ?Sized> DerefMut for DelegateAutoBorrow<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

pub macro delegate($(
	$(#[$attr:meta])*
	$vis:vis trait $trait_name:ident($trait_name_mut:ident)
		$(::<
			$(
				$($generic_lt:lifetime)?
				$($generic_para:ident)?
			),*
			$(,)?
		>)?
		::$fn_name:ident(
			$($arg_name:ident: $arg_ty:ty),*
			$(,)?
		) $(-> $ret_ty:ty)?
	$(where {$($where:tt)*})?;
)*) {$(
	// TODO: Validate generic params (ensure that we have commas in the right place)
	// TODO: Validate where clause

	$(#[$attr])*
	$vis trait $trait_name$(<$($($generic_lt)? $($generic_para)?),*>)?: Send
	$(where $($where)*)? {
		fn $fn_name(&self, $($arg_name: $arg_ty),*) $(-> $ret_ty)?;
	}

	$(#[$attr])*
	$vis trait $trait_name_mut$(<$($($generic_lt)? $($generic_para)?),*>)?: Send
	$(where $($where)*)? {
		fn $fn_name(&mut self, $($arg_name: $arg_ty),*) $(-> $ret_ty)?;
	}

	impl<
		__F: ?Sized + Send + Fn($($arg_ty),*) $(-> $ret_ty)?
		$($(, $($generic_lt)? $($generic_para)?)*)?
	>
		$trait_name$(<$($($generic_lt)? $($generic_para)?),*>)? for __F
		$(where $($where)*)?
	{
		fn $fn_name(&self, $($arg_name: $arg_ty),*) $(-> $ret_ty)? {
			(self)($($arg_name),*)
		}
	}

	impl<
		__F: ?Sized + Send + FnMut($($arg_ty),*) $(-> $ret_ty)?
		$($(, $($generic_lt)? $($generic_para)?)*)?
	>
		$trait_name_mut$(<$($($generic_lt)? $($generic_para)?),*>)? for __F
		$(where $($where)*)?
	{
		fn $fn_name(&mut self, $($arg_name: $arg_ty),*) $(-> $ret_ty)? {
			(self)($($arg_name),*)
		}
	}

	impl<
		__Target: ?Sized + $trait_name_mut$(<$($($generic_lt)? $($generic_para)?),*>)?,
		__Ptr: ?Sized + Send + for<'a> PromoteMut<'a, Value = __Target>
		$($(, $($generic_lt)? $($generic_para)?)*)?
	>
		$trait_name$(<$($($generic_lt)? $($generic_para)?),*>)? for DelegateAutoBorrow<__Ptr>
		$(where $($where)*)?
	{
		fn $fn_name(&self, $($arg_name: $arg_ty),*) $(-> $ret_ty)? {
			self.promote_mut().$fn_name($($arg_name),*)
		}
	}
)*}

// === Standard Delegates === //

delegate! {
	pub trait EventHandler(EventHandlerMut)::<E>::fire(s: Session, me: Entity, event: &mut E)
	where {
		E: ?Sized,
	};

	pub trait EventHandlerOnce(EventHandlerOnceMut)::<E>::fire(s: Session, me: Entity, event: E);

	pub trait Factory(FactoryMut)::<A, O>::create(s: Session, args: A) -> O
	where {
		O: core::fmt::Debug,
	};
}

// Multiplexing
#[derive(Debug, Clone)]
pub struct Multiplex<I>(pub I);

impl<I> From<I> for Multiplex<I> {
	fn from(iter: I) -> Self {
		Self(iter)
	}
}

impl<E, T, I> EventHandler<E> for Multiplex<I>
where
	E: ?Sized,
	T: EventHandlerMut<E>,
	I: Clone + IntoIterator<Item = (Entity, T)> + Send,
{
	fn fire(&self, s: Session, _: Entity, event: &mut E) {
		for (me, mut handler) in self.0.clone() {
			handler.fire(s, me, event);
		}
	}
}

impl<E, T, I> EventHandlerMut<E> for Multiplex<I>
where
	E: ?Sized,
	T: EventHandlerMut<E>,
	I: Clone + IntoIterator<Item = (Entity, T)> + Send,
{
	fn fire(&mut self, s: Session, _: Entity, event: &mut E) {
		for (me, mut handler) in self.0.clone() {
			handler.fire(s, me, event);
		}
	}
}
