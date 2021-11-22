//! Error reporting built of the Rust standard library [Error] trait.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

pub type AnyResult<T> = anyhow::Result<T>;
pub type AnyError = anyhow::Error;

pub trait ErrorFormatExt {
	fn format_error(&self, include_backtrace: bool) -> FormattedError<Self>;
}

impl<T: ?Sized + Error> ErrorFormatExt for T {
	fn format_error(&self, include_backtrace: bool) -> FormattedError<Self> {
		FormattedError {
			target: self,
			include_backtrace,
		}
	}
}

pub struct FormattedError<'a, T: ?Sized> {
	target: &'a T,
	include_backtrace: bool,
}

impl<T: ?Sized + Error> Display for FormattedError<'_, T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		let target = self.target;

		// Write context
		writeln!(f, "Error: {}", target)?;

		// Write cause chain
		// (we iterate manually instead of using `anyhow::Chain` because it consumes a `&dyn Error`.
		{
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
		}

		// Potentially include backtrace
		if self.include_backtrace {
			if let Some(backtrace) = target.backtrace() {
				writeln!(f, "\nBacktrace:")?;
				writeln!(f, "{}", backtrace)?;
			}
		}

		Ok(())
	}
}

/// A version of [anyhow::Context] for [Result] only. Supports producing context from an error value.
pub trait ResultExt<T, E> {
	fn unwrap_pretty(self) -> T;

	fn with_context_map<C, F>(self, f: F) -> Result<T, AnyError>
	where
		C: Display + Send + Sync + 'static,
		F: FnOnce(&E) -> C;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
	E: Error + Send + Sync + 'static,
{
	fn unwrap_pretty(self) -> T {
		match self {
			Ok(val) => val,
			Err(err) => panic!("{}", err.format_error(true)),
		}
	}

	fn with_context_map<C, F>(self, f: F) -> Result<T, AnyError>
	where
		C: Display + Send + Sync + 'static,
		F: FnOnce(&E) -> C,
	{
		self.map_err(|err| {
			let ctx = f(&err);
			AnyError::new(err).context(ctx)
		})
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
