use crate::provider::{Comp, Key, Provider, ProviderExt};

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

pub type ObjAncestry<'obj> = AncestryNode<'obj, &'obj dyn Provider>;

impl<'obj> ProviderExt for ObjAncestry<'obj> {
    type Obj = dyn Provider + 'obj;

    fn try_fetch_key<T: ?Sized>(&self, key: Key<T>) -> Option<Comp<Self::Obj, T>> {
        for ancestor in self.ancestors() {
            if let Some(component) = ancestor.try_fetch_key(key) {
                return Some(component);
            }
        }
        None
    }
}
