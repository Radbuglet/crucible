use hashbrown::raw::{RawIter, RawTable};
use std::borrow::Borrow;
use std::collections::hash_map::{DefaultHasher, RandomState};
use std::fmt::Display;
use std::hash::{BuildHasher, Hasher};
use std::marker::PhantomData;
use std::{fmt::Debug, num::NonZeroUsize};

pub struct Interner {
	/// A map from [str] (and its associated hash) to [Intern], with the [str] being derived from
	/// the [Intern].
	map: RawTable<(u64, Intern)>,

	/// The backing container for all our strings. A sequence of bytes is valid UTF-8 if and only if
	/// it is surrounded by `0xFF` bytes (which are guaranteed to never be present in valid Unicode
	/// streams).
	data: Vec<u8>,

	/// The hash builder used to generate hashes for the map.
	hash_builder: RandomState,
}

impl Debug for Interner {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		struct InternerVis<'a>(&'a Interner);

		impl Debug for InternerVis<'_> {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				let mut builder = f.debug_set();
				let interner = self.0;

				for intern in interner.interns() {
					builder.entry(&interner.decode(intern));
				}

				builder.finish()
			}
		}

		f.debug_struct("Interner")
			.field("interns", &InternerVis(self))
			.finish_non_exhaustive()
	}
}

impl Default for Interner {
	fn default() -> Self {
		Self {
			map: RawTable::new(),
			data: vec![0xFF],
			hash_builder: RandomState::new(),
		}
	}
}

impl Interner {
	pub fn begin_intern(&mut self) -> InternBuilder<'_> {
		let hasher = self.hash_builder.build_hasher();
		let start = self.data.len();

		InternBuilder(Some(InternBuilderInner {
			interner: self,
			start,
			hasher,
		}))
	}

	pub fn intern_str<S: Borrow<str>>(&mut self, str: S) -> Intern {
		self.begin_intern().with_str(str).finish()
	}

	pub fn intern_char(&mut self, char: char) -> Intern {
		self.begin_intern().with_char(char).finish()
	}

	pub fn intern_fmt<F: Display>(&mut self, text: F) -> Intern {
		self.begin_intern().with_fmt(text).finish()
	}

	pub fn decode_bytes(&self, intern: Intern) -> &[u8] {
		// Validate left delimiter
		assert_eq!(self.data[intern.start.get() - 1], 0xFF);
		let bytes = &self.data[intern.start.get()..];

		// Validate right delimiter
		assert_eq!(bytes[intern.len], 0xFF);
		let bytes = &bytes[..intern.len];

		bytes
	}

	pub fn decode(&self, intern: Intern) -> &str {
		let bytes = self.decode_bytes(intern);

		unsafe {
			// Safety: structure invariants assert that the `0xFF` delimiters indicate that a section
			// is valid Unicode.
			safer_utf8_to_str(bytes)
		}
	}

	pub fn interns(&self) -> InternIter<'_> {
		InternIter::new(&self.map)
	}
}

#[derive(Clone)]
pub struct InternIter<'a> {
	_ty: PhantomData<&'a RawTable<(u64, Intern)>>,
	iter: RawIter<(u64, Intern)>,
}

impl<'a> InternIter<'a> {
	fn new(table: &'a RawTable<(u64, Intern)>) -> Self {
		Self {
			_ty: PhantomData,
			iter: unsafe {
				// Safety: `_ty` ensures that `iter` does not outlive the provided `table` instance.
				table.iter()
			},
		}
	}
}

impl<'a> Iterator for InternIter<'a> {
	type Item = Intern;

	fn next(&mut self) -> Option<Self::Item> {
		let bucket = self.iter.next()?;
		let (_, intern) = *unsafe {
			// Safety: the reference rules of table buckets are inherited from the table reference.
			bucket.as_ref()
		};
		Some(intern)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let len = self.iter.len();
		(len, Some(len))
	}
}

impl<'a> ExactSizeIterator for InternIter<'a> {}

pub struct InternBuilder<'a>(Option<InternBuilderInner<'a>>);

impl Debug for InternBuilder<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let me = self.0.as_ref().unwrap();
		let bytes = &me.interner.data[me.start..];

		let text = unsafe {
			// Safety: We ensure that our intern's Unicode bytestream is valid at all times by pushing
			// codepoints atomically.
			safer_utf8_to_str(bytes)
		};

		f.debug_struct("InternBuilder")
			.field("buffer", &text)
			.finish_non_exhaustive()
	}
}

struct InternBuilderInner<'a> {
	/// A reference to the interner in which we're building our intern.
	interner: &'a mut Interner,

	/// An index to the first actual byte of our character byte stream (one past the `0xFF` delimiter).
	start: usize,

	/// An ongoing hashing session to hash our string.
	hasher: DefaultHasher,
}

impl InternBuilder<'_> {
	fn internal_push_bytes(&mut self, bytes: &[u8]) {
		let me = self.0.as_mut().unwrap();

		// Safety: panicking in the middle of an extension would be *really bad*. Luckily,
		// `extend_from_slice` reserves its capacity up-front so it either fully commits the object
		// or doesn't at all.
		me.interner.data.extend_from_slice(bytes);

		// This shouldn't panic either but it honestly doesn't really matter if it does; we've
		// already preserved all structure invariants. The worst thing it can do is cause an intern
		// to be duplicated, which isn't all that bad.
		me.hasher.write(bytes);
	}

	pub fn push_char(&mut self, char: char) {
		let mut dst = [0; 4];
		char.encode_utf8(&mut dst);
		self.internal_push_bytes(&dst[0..char.len_utf8()]);
	}

	pub fn with_char(mut self, char: char) -> Self {
		self.push_char(char);
		self
	}

	pub fn push_str<S: Borrow<str>>(&mut self, str: S) {
		self.internal_push_bytes(str.borrow().as_bytes());
	}

	pub fn with_str<S: Borrow<str>>(mut self, str: S) -> Self {
		self.push_str(str);
		self
	}

	pub fn push_fmt<F: Display>(&mut self, text: F) {
		self.push_str(text.to_string());
	}

	pub fn with_fmt<F: Display>(mut self, text: F) -> Self {
		self.push_fmt(text);
		self
	}

	pub fn finish(mut self) -> Intern {
		let InternBuilderInner {
			interner,
			start,
			hasher,
		} = self.0.take().unwrap();

		// Ensure that we haven't already interned this string.
		let bytes = &interner.data[start..];
		let hash = hasher.finish();

		if let Some((_, intern)) = interner.map.get(hash, |(entry_hash, entry_intern)| {
			if hash != *entry_hash {
				return false;
			}

			let entry_bytes = interner.decode_bytes(*entry_intern);

			bytes == entry_bytes
		}) {
			interner.data.truncate(start);
			*intern
		} else {
			let len = bytes.len();
			let intern = Intern {
				start: NonZeroUsize::new(start).unwrap(),
				len,
			};
			interner.data.push(0xFF);
			interner.map.insert(hash, (hash, intern), |(hash, _)| *hash);
			intern
		}
	}
}

impl Drop for InternBuilder<'_> {
	fn drop(&mut self) {
		if let Some(me) = &mut self.0 {
			me.interner.data.truncate(me.start);
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Intern {
	/// The index of the start byte (one past the `0xFF` delimiter).
	start: NonZeroUsize,

	/// The length in bytes of the inner character sequence (does not include either delimiter).
	len: usize,
}

unsafe fn safer_utf8_to_str(bytes: &[u8]) -> &str {
	debug_assert!(std::str::from_utf8(bytes).is_ok());

	// Safety: provided by caller
	std::str::from_utf8_unchecked(bytes)
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn basic_interning() {
		let mut interner = Interner::default();

		let whee = interner.intern_str("whee");
		let woo = interner.intern_str("woo");
		let waz = interner.intern_str("waz");
		let whee2 = interner.intern_str("whee");
		let big_str = interner.intern_str("this is a very big string");
		let waz2 = interner.intern_str("waz");
		let big_str2 = interner.intern_str("this is a very big string");
		let whee3 = interner
			.begin_intern()
			.with_char('w')
			.with_str("he")
			.with_fmt("e")
			.finish();

		assert_eq!(whee, whee2);
		assert_ne!(woo, waz);
		assert_eq!(waz, waz2);
		assert_eq!(big_str, big_str2);
		assert_eq!(whee, whee3);

		assert_eq!(interner.interns().len(), 4);
	}
}
