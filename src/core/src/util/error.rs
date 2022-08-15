//! Error reporting built off the Rust standard library [Error] trait.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::std_traits::ResultLike;

// === Standard Error Extensions === //

pub trait ErrorFormatExt: Error {
	fn format_error(&self) -> FormattedError<Self>;

	fn raise(&self) -> ! {
		panic!("{}", self.format_error());
	}

	fn log(&self) {
		log::error!("{}", self.format_error());
	}
}

impl<T: ?Sized + Error> ErrorFormatExt for T {
	fn format_error(&self) -> FormattedError<Self> {
		FormattedError(self)
	}
}

pub struct FormattedError<'a, T: ?Sized>(pub &'a T);

impl<T: ?Sized> Copy for FormattedError<'_, T> {}

impl<T: ?Sized> Clone for FormattedError<'_, T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized + Error> Display for FormattedError<'_, T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		let target = self.0;

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

pub trait ResultExt: ResultLike {
	fn unwrap_pretty(self) -> Self::Success;
	fn log(self) -> Option<Self::Success>;
}

impl<T, E: Error> ResultExt for Result<T, E> {
	fn unwrap_pretty(self) -> T {
		match self {
			Ok(val) => val,
			Err(err) => err.raise(),
		}
	}

	fn log(self) -> Option<T> {
		match self {
			Ok(val) => Some(val),
			Err(err) => {
				err.log();
				None
			}
		}
	}
}

pub type AnyhowErrorBoxed = Box<AnyhowErrorInner>;
pub type AnyhowErrorInner = dyn Error + Send + Sync + 'static;

pub trait AnyhowConvertExt {
	type StdError;

	fn into_std_error(self) -> Self::StdError;
}

impl<T> AnyhowConvertExt for anyhow::Result<T> {
	type StdError = Result<T, AnyhowErrorBoxed>;

	fn into_std_error(self) -> Self::StdError {
		self.map_err(|err| err.into())
	}
}
