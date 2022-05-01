//! Error reporting built of the Rust standard library [Error] trait.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

pub trait ErrorFormatExt {
	fn format_error(&self) -> FormattedError<Self>;
}

impl<T: ?Sized + Error> ErrorFormatExt for T {
	fn format_error(&self) -> FormattedError<Self> {
		FormattedError { target: self }
	}
}

pub struct FormattedError<'a, T: ?Sized> {
	target: &'a T,
}

impl<T: ?Sized + Error> Display for FormattedError<'_, T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		let target = self.target;

		// Write context
		writeln!(f, "Error: {}", target)?;

		// Write cause chain
		// (we iterate manually instead of using `anyhow::Chain` because it consumes a `&dyn Error`.
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

/// A version of [anyhow::Context] for [Result] only. Supports producing context from an error value.
pub trait ResultExt<T, E: Error> {
	fn unwrap_pretty(self) -> T;
}

impl<T, E: Error> ResultExt<T, E> for Result<T, E> {
	fn unwrap_pretty(self) -> T {
		match self {
			Ok(val) => val,
			Err(err) => panic!("{}", err.format_error()),
		}
	}
}

pub trait OkUnwrapExt {
	type Item;
	type Context;

	fn unwrap_or_panic<F, D>(self, err_gen: F) -> Self::Item
	where
		F: FnOnce(Self::Context) -> D,
		D: Display;
}

impl<T> OkUnwrapExt for Option<T> {
	type Item = T;
	type Context = ();

	fn unwrap_or_panic<F, D>(self, err_gen: F) -> Self::Item
	where
		F: FnOnce(Self::Context) -> D,
		D: Display,
	{
		match self {
			Some(value) => value,
			None => panic!("{}", err_gen(())),
		}
	}
}

impl<T, E> OkUnwrapExt for Result<T, E> {
	type Item = T;
	type Context = E;

	fn unwrap_or_panic<F, D>(self, err_gen: F) -> Self::Item
	where
		F: FnOnce(Self::Context) -> D,
		D: Display,
	{
		match self {
			Ok(value) => value,
			Err(err) => panic!("{}", err_gen(err)),
		}
	}
}
