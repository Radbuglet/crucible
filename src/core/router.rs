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

pub trait EventHandler {
    type Ancestor;
    type Event;

    fn handle(&mut self, tree: &AncestryNode<Self::Ancestor>, event: &Self::Event);
}

// TODO: Integration w/ game objects
