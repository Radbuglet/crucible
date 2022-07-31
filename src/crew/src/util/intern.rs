use hashbrown::raw::{RawIter, RawTable};
use std::{
	borrow::Borrow,
	collections::hash_map::{DefaultHasher, RandomState},
	fmt,
	hash::{BuildHasher, Hasher},
	marker::PhantomData,
};

#[derive(Default)]
pub struct Interner {
	/// A map from [str] (and its associated hash) to [Intern], with the [str] being derived from
	/// the [Intern].
	map: RawTable<(u64, Intern)>,

	/// The backing container for all our strings.
	data: String,

	/// The hash builder used to generate hashes for the map.
	hash_builder: RandomState,
}

impl fmt::Debug for Interner {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		struct InternerVis<'a>(&'a Interner);

		impl fmt::Debug for InternerVis<'_> {
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

	pub fn intern_fmt<F: fmt::Display>(&mut self, text: F) -> Intern {
		self.begin_intern().with_fmt(text).finish()
	}

	pub fn decode(&self, intern: Intern) -> &str {
		&self.data[intern.start..(intern.start + intern.len)]
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

impl fmt::Debug for InternBuilder<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let me = self.0.as_ref().unwrap();
		let text = &me.interner.data[me.start..];

		f.debug_struct("InternBuilder")
			.field("buffer", &text)
			.finish_non_exhaustive()
	}
}

struct InternBuilderInner<'a> {
	/// A reference to the interner in which we're building our intern.
	interner: &'a mut Interner,

	/// An index to the first byte of our character byte stream.
	start: usize,

	/// An ongoing hashing session to hash our string.
	hasher: DefaultHasher,
}

impl<'a> InternBuilder<'a> {
	fn unwrap_inner_mut(&mut self) -> &mut InternBuilderInner<'a> {
		self.0.as_mut().unwrap()
	}

	pub fn push_char(&mut self, char: char) {
		self.unwrap_inner_mut().interner.data.push(char)
	}

	pub fn with_char(mut self, char: char) -> Self {
		self.push_char(char);
		self
	}

	pub fn push_str<S: Borrow<str>>(&mut self, str: S) {
		self.unwrap_inner_mut().interner.data.push_str(str.borrow())
	}

	pub fn with_str<S: Borrow<str>>(mut self, str: S) -> Self {
		self.push_str(str);
		self
	}

	pub fn push_fmt<F: fmt::Display>(&mut self, text: F) {
		self.push_str(text.to_string());
	}

	pub fn with_fmt<F: fmt::Display>(mut self, text: F) -> Self {
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
		let text = &interner.data[start..];
		let hash = hasher.finish();

		if let Some((_, intern)) = interner.map.get(hash, |(entry_hash, entry_intern)| {
			if hash != *entry_hash {
				return false;
			}

			let entry_text = interner.decode(*entry_intern);
			text == entry_text
		}) {
			interner.data.truncate(start);
			*intern
		} else {
			let len = text.len();
			let intern = Intern { start, len };
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
	/// The index of the start byte.
	start: usize,

	/// The length in bytes of the character sequence.
	len: usize,
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
