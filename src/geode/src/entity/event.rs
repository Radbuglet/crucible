use std::cell::RefCell;

use super::entity::Entity;
use crate::core::session::Session;

pub macro delegate {
	// Muncher base case
	() => {},

	// Immutable
	(
		$(#[$attr:meta])*
		$vis:vis trait
			$name:ident
			$(::<$($generic_param:ident),*$(,)?>)?
			::
			$fn_name:ident
			$(<
				$($lt_decl:lifetime),*
				$(,)?
			>)?
		(
			&self,
			$($arg_name:ident: $arg_ty:ty),*
			$(,)?
		) $(-> $ret:ty)?;

		$($rest:tt)*
	) => {
		$(#[$attr:meta])*
		$vis trait $name $(<$($generic_param),*>)?: Send {
			fn $fn_name $(<$($lt_decl),*>)? (&self, $($arg_name: $arg_ty),*) $(-> $ret)?;
		}

		impl<F: Send $(,$($generic_param),*)?> $name $(<$($generic_param),*>)? for F
		where
			F: $(for<$($lt_decl),*>)? Fn($($arg_ty),*) $(-> $ret)?,
		{
			fn $fn_name $(<$($lt_decl),*>)? (&self, $($arg_name: $arg_ty),*) $(-> $ret)? {
				(self)($($arg_name),*)
			}
		}

		delegate!($($rest)*);
	},

	// Mutable
	(
		$(#[$attr:meta])*
		$vis:vis trait
			$name:ident
			$(::<$($generic_param:ident),*$(,)?>)?
			::
			$fn_name:ident
			$(<
				$($lt_decl:lifetime),*
				$(,)?
			>)?
		(
			&mut self,
			$($arg_name:ident: $arg_ty:ty),*
			$(,)?
		) $(-> $ret:ty)?;

		$($rest:tt)*
	) => {
		$(#[$attr:meta])*
		$vis trait $name $(<$($generic_param),*>)?: Send {
			fn $fn_name $(<$($lt_decl),*>)? (&mut self, $($arg_name: $arg_ty),*) $(-> $ret)?;
		}

		impl<F: Send $(,$($generic_param),*)?> $name $(<$($generic_param),*>)? for F
		where
			F: $(for<$($lt_decl),*>)? FnMut($($arg_ty),*) $(-> $ret)?,
		{
			fn $fn_name $(<$($lt_decl),*>)? (&mut self, $($arg_name: $arg_ty),*) $(-> $ret)? {
				(self)($($arg_name),*)
			}
		}

		delegate!($($rest)*);
	},
}

pub trait EventHandler<E: ?Sized> {
	fn fire(&self, event: &mut E);
}

pub trait EventHandlerMut<E: ?Sized> {
	fn fire(&mut self, event: &mut E);
}

impl<E: ?Sized, T: Fn(&mut E)> EventHandler<E> for T {
	fn fire(&self, event: &mut E) {
		(self)(event)
	}
}

impl<E: ?Sized, T: FnMut(&mut E)> EventHandlerMut<E> for T {
	fn fire(&mut self, event: &mut E) {
		(self)(event)
	}
}

impl<E: ?Sized, T: ?Sized + EventHandlerMut<E>> EventHandler<E> for RefCell<T> {
	fn fire(&self, event: &mut E) {
		self.borrow_mut().fire(event)
	}
}

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
	I: Clone + IntoIterator<Item = T>,
{
	fn fire(&self, event: &mut E) {
		for mut handler in self.0.clone() {
			handler.fire(event);
		}
	}
}

#[derive(Debug)]
pub struct EntityEvent<'s, 'e, E: ?Sized> {
	pub session: Session<'s>,
	pub target: Entity,
	pub event: &'e mut E,
}

#[derive(Debug)]
pub struct SessionEvent<'s, 'e, E: ?Sized> {
	pub session: Session<'s>,
	pub event: &'e mut E,
}

#[derive(Debug, Clone)]
pub struct BindEntityToHandler<H: ?Sized> {
	pub entity: Entity,
	pub handler: H,
}

impl<'s, 'e, H, E> EventHandlerMut<SessionEvent<'s, 'e, E>> for BindEntityToHandler<H>
where
	H: ?Sized + for<'s2, 'e2> EventHandlerMut<EntityEvent<'s2, 'e2, E>>,
	E: ?Sized,
{
	fn fire(&mut self, event: &mut SessionEvent<'s, 'e, E>) {
		self.handler.fire(&mut EntityEvent {
			session: event.session,
			target: self.entity,
			event: &mut event.event,
		});
	}
}

impl<'s, 'e, H, E> EventHandler<SessionEvent<'s, 'e, E>> for BindEntityToHandler<H>
where
	H: ?Sized + for<'s2, 'e2> EventHandler<EntityEvent<'s2, 'e2, E>>,
	E: ?Sized,
{
	fn fire(&self, event: &mut SessionEvent<'s, 'e, E>) {
		self.handler.fire(&mut EntityEvent {
			session: event.session,
			target: self.entity,
			event: &mut event.event,
		});
	}
}
