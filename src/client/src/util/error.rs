//! Helper methods for `std::error::Error`.

use std::error::Error;

pub type AnyResult<T> = Result<T, Box<dyn Error>>;

pub trait ResultExt<T, E> {
    fn unwrap_pretty(self) -> T;
}

impl<T, E: Error> ResultExt<T, E> for Result<T, E> {
    fn unwrap_pretty(self) -> T {
        match self {
            Ok(success) => success,
            Err(error) => panic!("Unwrapped error: {}", error),
        }
    }
}
