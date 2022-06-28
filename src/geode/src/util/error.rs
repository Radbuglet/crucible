//! Error reporting built of the Rust standard library [Error] trait.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

use derive_where::derive_where;

pub trait ErrorFormatExt: Error {
	fn format_error(&self) -> FormattedError<Self>;

	fn raise(&self) -> ! {
		panic!("{}", self.format_error());
	}
}

impl<T: ?Sized + Error> ErrorFormatExt for T {
	fn format_error(&self) -> FormattedError<Self> {
		FormattedError { target: self }
	}
}

#[derive_where(Clone)]
pub struct FormattedError<'a, T: ?Sized> {
	target: &'a T,
}

impl<T: ?Sized + Error> Display for FormattedError<'_, T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		let target = self.target;

		// Write context
		writeln!(f, "Error: {}", target)?;

		// Write cause chain
		let mut cause_iter = target.source();
		if cause_iter.is_some() {
			writeln!(f, "\nCaused by:")?;
		}

		while let Some(cause) = cause_iter {
			for line in cause.to_string().lines() {
				writeln!(f, "\t{}", line)?;
			}
			cause_iter = cause.source();
		}

		Ok(())
	}
}

pub trait ResultExt<T, E: Error> {
	fn unwrap_pretty(self) -> T;
}

impl<T, E: Error> ResultExt<T, E> for Result<T, E> {
	fn unwrap_pretty(self) -> T {
		match self {
			Ok(val) => val,
			Err(err) => err.raise(),
		}
	}
}

pub trait UnwrapExt<T, E> {
	fn unwrap_using<F, EF>(self, f: F) -> T
	where
		F: FnMut(E) -> EF,
		EF: Display;
}

impl<T> UnwrapExt<T, ()> for Option<T> {
	fn unwrap_using<F, EF>(self, mut f: F) -> T
	where
		F: FnMut(()) -> EF,
		EF: Display,
	{
		match self {
			Some(value) => value,
			None => panic!("{}", f(())),
		}
	}
}

impl<T, E> UnwrapExt<T, E> for Result<T, E> {
	fn unwrap_using<F, EF>(self, mut f: F) -> T
	where
		F: FnMut(E) -> EF,
		EF: Display,
	{
		match self {
			Ok(value) => value,
			Err(err) => panic!("{}", f(err)),
		}
	}
}
