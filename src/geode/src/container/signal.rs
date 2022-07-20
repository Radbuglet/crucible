use std::cell::{Cell, RefCell};

use crucible_core::marker::PhantomInvariant;

use crate::{
	core::{
		obj::{Lock, Obj, ObjCtorExt, ObjPointee},
		owned::Owned,
		session::Session,
	},
	entity::{entity::Entity, event::EventHandler},
};

type HandlerList<T> = Vec<Owned<Obj<Connection<T>>>>;

pub struct Signal<T: ?Sized + ObjPointee> {
	lock: Lock,
	connections: RefCell<HandlerList<T>>,
}

impl<T: ?Sized + ObjPointee> Signal<T> {
	pub fn new(lock: Lock) -> Self {
		Self {
			lock,
			connections: Default::default(),
		}
	}

	pub fn connect(
		&self,
		s: Session,
		entity: Entity,
		handler: Obj<T>,
		conn_info: ConnectionInfo<T>,
	) -> ConnectionHandle<T> {
		let mut handlers = self.connections.borrow_mut();

		let (conn_guard, conn) = Connection {
			entity,
			handler,
			index: Cell::new(handlers.len()),
			#[cfg(debug_assertions)]
			debug_is_weak: conn_info.weakly_connect,
		}
		.box_obj_in(s, self.lock)
		.to_guard_ref_pair();

		handlers.push(conn_guard);

		ConnectionHandle { obj: conn }
	}

	fn index_if_connected(
		s: Session,
		handlers: &HandlerList<T>,
		conn: ConnectionHandle<T>,
	) -> Option<usize> {
		let p_conn = conn.obj.get(s);
		let index = p_conn.index.get();

		if matches!(
			handlers.get(index),
			Some(owned_conn) if owned_conn.weak_copy() == conn.obj && p_conn.handler.is_alive_now(s)
		) {
			Some(index)
		} else {
			None
		}
	}

	pub fn is_connected(&self, s: Session, conn: ConnectionHandle<T>) -> bool {
		Self::index_if_connected(s, &self.connections.borrow(), conn).is_some()
	}

	pub fn disconnect(&self, s: Session, conn: ConnectionHandle<T>) -> bool {
		let mut handlers = self.connections.borrow_mut();

		let conn_index = match Self::index_if_connected(s, &mut handlers, conn) {
			Some(index) => index,
			None => return false,
		};

		let _ = handlers.swap_remove(conn_index);

		if let Some(dirty) = handlers.last() {
			dirty.get(s).index.set(handlers.len() - 1);
		}

		true
	}

	pub fn clear(&self, _s: Session) {
		self.connections.borrow_mut().clear();
	}
}

impl<E, T: ?Sized + ObjPointee> EventHandler<E> for Signal<T>
where
	E: ?Sized,
	T: EventHandler<E>,
{
	fn fire(&self, s: Session, _: Entity, event: &mut E) {
		self.connections.borrow_mut().retain_mut(|conn| {
			let p_conn = conn.get(s);
			if let Ok(handler) = p_conn.handler.weak_get(s) {
				handler.fire(s, p_conn.entity, event);
				true
			} else {
				#[cfg(debug_assertions)]
				{
					assert!(
						p_conn.debug_is_weak,
						"A non-weak signal has just been disconnected."
					);
				}
				false
			}
		});
	}
}

struct Connection<T: ?Sized + ObjPointee> {
	entity: Entity,
	handler: Obj<T>,
	index: Cell<usize>,
	#[cfg(debug_assertions)]
	debug_is_weak: bool,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct ConnectionInfo<T: ?Sized + ObjPointee> {
	_non_exhaustive: PhantomInvariant<T>,
	pub weakly_connect: bool,
}

pub struct ConnectionHandle<T: ?Sized + ObjPointee> {
	obj: Obj<Connection<T>>,
}
