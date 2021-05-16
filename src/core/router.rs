use std::any::Any;
use super::game_object::{new_key, Key, GameObject, GameObjectExt};

// === Ancestry core === //

pub struct AncestryNode<'a, T> {
    pub parent: Option<&'a AncestryNode<'a, T>>,
    pub value: T,
}

impl<'a, T> AncestryNode<'a, T> {
    pub fn root(value: T) -> Self {
        Self {
            parent: None,
            value,
        }
    }

    pub fn child(&'a self, value: T) -> Self {
        Self {
            parent: Some(self),
            value,
        }
    }

    pub fn ancestors(&self) -> AncestryIter<T> {
        AncestryIter {
            curr: Some(self),
        }
    }
}

pub struct AncestryIter<'a, T> {
    curr: Option<&'a AncestryNode<'a, T>>,
}

impl<'a, T> Iterator for AncestryIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.curr.and_then(|curr| curr.parent);

        std::mem::replace(&mut self.curr, next)
            .map(|curr| &curr.value)
    }
}

// === Game Object Routing === //

pub type GObjAncestry<'a> = AncestryNode<'a, &'a dyn GameObject>;

impl<'a> GObjAncestry<'a> {
    pub fn try_get_obj_attributed<T: ?Sized>(&self, key: Key<T>) -> Option<(&T, &dyn GameObject)> {
        for ancestor in self.ancestors() {
            if let Some(component) = ancestor.try_get(key) {
                return Some((component, *ancestor))
            }
        }
        None
    }

    pub fn try_get_obj<T: ?Sized>(&self, key: Key<T>) -> Option<&T> {
        self.try_get_obj_attributed(key)
            .map(|(comp, _)| comp)
    }

    pub fn get_obj<T: ?Sized>(&self, key: Key<T>) -> &T {
        self.try_get_obj_attributed(key)
            .unwrap().0
    }

    pub fn has_obj<T: ?Sized>(&self, key: Key<T>) -> bool {
        self.try_get_obj_attributed(key)
            .is_some()
    }
}

pub const EVENT_HANDLER_KEY: Key<dyn EventHandler> = new_key!(dyn EventHandler);

pub trait EventHandler {
    fn handle(&self, ancestry: &GObjAncestry, event: &dyn Any);
}

pub fn route_to_obj(target: &dyn GameObject, ancestry: &GObjAncestry, event: &dyn Any) {
    target.get(EVENT_HANDLER_KEY)
        .handle(&ancestry.child(target), event);
}
