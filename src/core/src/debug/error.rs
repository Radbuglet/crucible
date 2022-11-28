//! Error reporting built off the Rust standard library [Error] trait.

use std::{
	error::Error,
	fmt::{Display, Formatter, Result as FmtResult},
	thread::panicking,
};

use derive_where::derive_where;

use crate::lang::std_traits::ResultLike;

// === Standard Error Extensions === //

pub trait ErrorFormatExt: Error {
	fn format_error(&self) -> FormattedError<Self> {
		FormattedError(self)
	}

	fn raise(&self) -> ! {
		panic!("{}", self.format_error());
	}

	fn log(&self) {
		log::error!("{}", self.format_error());
	}

	fn raise_unless_panicking(&self) {
		if !panicking() {
			self.raise();
		} else {
			self.log();
		}
	}
}

impl<T: ?Sized + Error> ErrorFormatExt for T {}

#[derive_where(Copy, Clone)]
pub struct FormattedError<'a, T: ?Sized>(pub &'a T);

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
	fn unwrap_unless_panicking(self) -> Option<Self::Success>;
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

	fn unwrap_unless_panicking(self) -> Option<T> {
		match self {
			Ok(val) => Some(val),
			Err(err) => {
				err.raise_unless_panicking();
				None
			}
		}
	}
}

// === Anyhow conversions === //

pub type AnyhowErrorBoxed = Box<AnyhowErrorInner>;
pub type AnyhowErrorInner = dyn Error + Send + Sync + 'static;

pub trait AnyhowConvertExt {
	type AsStd;

	fn into_std_error(self) -> Self::AsStd;
}

impl<T> AnyhowConvertExt for anyhow::Result<T> {
	type AsStd = Result<T, AnyhowErrorBoxed>;

	fn into_std_error(self) -> Self::AsStd {
		self.map_err(|err| err.into())
	}
}
