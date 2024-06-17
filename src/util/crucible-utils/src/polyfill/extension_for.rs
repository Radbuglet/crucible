pub trait Extension {
    type Me: ?Sized;

    fn value(&self) -> &Self::Me;

    fn value_mut(&mut self) -> &mut Self::Me;

    fn into_value(self) -> Self::Me
    where
        Self: Sized;
}

impl<T: ?Sized> Extension for T {
    type Me = Self;

    fn value(&self) -> &Self::Me {
        self
    }

    fn value_mut(&mut self) -> &mut Self::Me {
        self
    }

    fn into_value(self) -> Self::Me
    where
        Self: Sized,
    {
        self
    }
}
