use std::cell::{Ref, RefMut};

use crate::debug::userdata::UserdataValue;

use super::{
	celled::{CelledStorage, CelledStorageView},
	core::{Entity, Storage},
};

pub type Userdata = Box<dyn UserdataValue>;

impl Storage<Userdata> {
	pub fn get_downcast<T: 'static>(&self, entity: Entity) -> &T {
		self[entity].downcast_ref()
	}

	pub fn get_downcast_mut<T: 'static>(&mut self, entity: Entity) -> &mut T {
		self[entity].downcast_mut()
	}
}

impl CelledStorage<Userdata> {
	pub fn get_downcast<T: 'static>(&self, entity: Entity) -> &T {
		self.get(entity).downcast_ref()
	}

	pub fn get_downcast_mut<T: 'static>(&mut self, entity: Entity) -> &mut T {
		self.get_mut(entity).downcast_mut()
	}
}

impl CelledStorageView<Userdata> {
	pub fn borrow_downcast<T: 'static>(&self, entity: Entity) -> Ref<T> {
		Ref::map(self.borrow(entity), |comp| comp.downcast_ref())
	}

	pub fn get_downcast_mut<T: 'static>(&self, entity: Entity) -> RefMut<T> {
		RefMut::map(self.borrow_mut(entity), |comp| comp.downcast_mut())
	}
}
