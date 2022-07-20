use std::cell::RefCell;

use super::entity::Entity;
use crate::core::session::Session;

pub trait EventHandler<E: ?Sized>: Send {
	fn fire(&self, s: Session, me: Entity, event: &mut E);
}

pub trait EventHandlerMut<E: ?Sized>: Send {
	fn fire(&mut self, s: Session, me: Entity, event: &mut E);
}

pub trait EventHandlerTerminal<E>: Send {
	fn fire(&self, s: Session, me: Entity, event: E);
}

pub trait EventHandlerTerminalMut<E>: Send {
	fn fire(&mut self, s: Session, me: Entity, event: E);
}

// Closure derivations
impl<E: ?Sized, T: Fn(Session, Entity, &mut E) + Send> EventHandler<E> for T {
	fn fire(&self, s: Session, me: Entity, event: &mut E) {
		(self)(s, me, event)
	}
}

impl<E: ?Sized, T: FnMut(Session, Entity, &mut E) + Send> EventHandlerMut<E> for T {
	fn fire(&mut self, s: Session, me: Entity, event: &mut E) {
		(self)(s, me, event)
	}
}

impl<E, T: Fn(Session, Entity, E) + Send> EventHandlerTerminal<E> for T {
	fn fire(&self, s: Session, me: Entity, event: E) {
		(self)(s, me, event)
	}
}

impl<E, T: FnMut(Session, Entity, E) + Send> EventHandlerTerminalMut<E> for T {
	fn fire(&mut self, s: Session, me: Entity, event: E) {
		(self)(s, me, event)
	}
}

// RefCell derivation
impl<E: ?Sized, T: ?Sized + EventHandlerMut<E>> EventHandler<E> for RefCell<T> {
	fn fire(&self, s: Session, me: Entity, event: &mut E) {
		self.borrow_mut().fire(s, me, event)
	}
}

impl<E, T: ?Sized + EventHandlerTerminalMut<E>> EventHandlerTerminal<E> for RefCell<T> {
	fn fire(&self, s: Session, me: Entity, event: E) {
		self.borrow_mut().fire(s, me, event)
	}
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
