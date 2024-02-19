mod extension_for {
    pub trait Sealed<T: ?Sized> {}
}

pub trait ExtensionFor<T: ?Sized>: extension_for::Sealed<T> {
    fn v(&self) -> &T;

    fn v_mut(&mut self) -> &mut T;

    fn into_v(self) -> T
    where
        Self: Sized;
}

impl<T: ?Sized> extension_for::Sealed<T> for T {}

impl<T: ?Sized> ExtensionFor<T> for T {
    fn v(&self) -> &T {
        self
    }

    fn v_mut(&mut self) -> &mut T {
        self
    }

    fn into_v(self) -> T
    where
        Self: Sized,
    {
        self
    }
}
