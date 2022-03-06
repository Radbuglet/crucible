use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Entity {
    pub(crate) slot: usize,
    pub(crate) gen: NonZeroU64,
}

impl Entity {
    pub fn slot(&self) -> usize {
        self.slot
    }

    pub fn gen(&self) -> NonZeroU64 {
        self.gen
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct EntityHandle {
    raw: Entity,
}

#[derive(Debug, Copy, Clone)]
pub struct ComponentPair<'a, T: ?Sized> {
    entity: Entity,
    comp: &'a T,
}

impl<'a, T: ?Sized> ComponentPair<'a, T> {
    pub fn entity_id(&self) -> Entity {
        self.entity
    }

    pub fn component(&self) -> &'a T {
        self.comp
    }
}

impl<'a, T: ?Sized> Deref for ComponentPair<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.comp
    }
}

#[derive(Debug)]
pub struct ComponentPairMut<'a, T: ?Sized> {
    pub(crate) entity: Entity,
    pub(crate) comp: &'a mut T,
}

impl<'a, T: ?Sized> ComponentPairMut<'a, T> {
    pub fn entity(&self) -> Entity {
        self.entity
    }

    pub fn component(&self) -> &T {
        self.comp
    }

    pub fn component_mut(&mut self) -> &mut T {
        self.comp
    }

    pub fn to_component(self) -> &'a mut T {
        self.comp
    }

    pub fn downgrade(&self) -> ComponentPair<'_, T> {
        ComponentPair {
            entity: self.entity,
            comp: self.comp,
        }
    }
}

impl<'a, T: ?Sized> Deref for ComponentPairMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.comp
    }
}

impl<'a, T: ?Sized> DerefMut for ComponentPairMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.comp
    }
}
