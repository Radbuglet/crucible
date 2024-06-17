use super::extension_for::Extension;

pub trait OptionExt: Extension<Me = Option<Self::Inner>> {
    type Inner;

    fn is_none_or(&self, f: impl FnOnce(&Self::Inner) -> bool) -> bool {
        match self.value() {
            Some(v) => f(v),
            None => true,
        }
    }
}

impl<T> OptionExt for Option<T> {
    type Inner = T;
}
