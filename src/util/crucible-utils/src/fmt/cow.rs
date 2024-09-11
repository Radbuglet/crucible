use std::{borrow::Cow, fmt};

pub trait CowDisplay {
    fn fmt_cow(&self) -> Cow<'static, str>;
}

impl CowDisplay for &'static str {
    fn fmt_cow(&self) -> Cow<'static, str> {
        Cow::Borrowed(*self)
    }
}

impl CowDisplay for fmt::Arguments<'_> {
    fn fmt_cow(&self) -> Cow<'static, str> {
        match self.as_str() {
            Some(value) => Cow::Borrowed(value),
            None => Cow::Owned(self.to_string()),
        }
    }
}
