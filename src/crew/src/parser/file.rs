use super::generic::ForkableCursor;
use crate::parser::generic::{Atom, Cursor};
use crate::util::iter_magic::limit_len;
use crate::util::obj::Entity;
use std::cmp::Ordering;
use std::ops::{Bound, Range, RangeBounds};
use std::str::from_utf8 as str_from_utf8;
use thiserror::Error;

// === Source file representations === //

/// Optional file descriptor metadata to help users find the corresponding source file.
#[derive(Debug, Clone)]
pub struct SourceFileInfo {
	pub name: String,
}

/// Optional object span metadata to help users find the location of the element in its source file.
#[derive(Debug, Clone)]
pub struct Spanned {
	pub span: Span,
}

/// A file whose contents have been loaded into memory.
#[derive(Debug, Clone)]
pub struct LoadedFile {
	pub file_desc: Entity,
	pub contents: Vec<u8>,
}

impl LoadedFile {
	pub fn reader(&self) -> FileReader<'_> {
		FileReader {
			file_desc: self.file_desc,
			codepoints: CodepointReader::new(&self.contents),
			latest_pos: (0, FilePos::START),
			next_pos: FilePos::START,
		}
	}
}

// === Codepoint reader === //

/// A reader that reads codepoints from a byte stream.
#[derive(Debug, Clone)]
pub struct CodepointReader<'a> {
	/// The contents of the file.
	pub contents: &'a [u8],

	/// The index of the latest character we parsed.
	pub latest_index: usize,

	/// The index of start of the next codepoint to be read. This index can be one past the end of
	/// the buffer, indicating that we consumed the entire stream.
	pub next_index: usize,
}

impl<'a> CodepointReader<'a> {
	pub fn new(contents: &'a [u8]) -> Self {
		Self::new_range(contents, ..)
	}

	pub fn new_range<R: RangeBounds<usize>>(contents: &'a [u8], range: R) -> Self {
		let offset = match range.start_bound() {
			Bound::Unbounded => 0,
			Bound::Included(index) => *index,
			Bound::Excluded(index) => *index + 1,
		};

		Self {
			contents: &contents[(Bound::Unbounded, range.end_bound().cloned())],
			latest_index: offset,
			next_index: offset,
		}
	}
}

impl<'a> Cursor for CodepointReader<'a> {
	type Loc = usize;
	type Atom = Result<Option<char>, UnicodeParseError<'a>>;

	fn latest(&self) -> (Self::Loc, Self::Atom) {
		if self.latest_index == self.next_index {
			return (self.latest_index, Ok(None));
		}

		let bytes = &self.contents[self.latest_index..self.next_index];
		let atom = match str_from_utf8(bytes) {
			Ok(str) => Ok(str.chars().next()),
			Err(err) => Err(UnicodeParseError {
				offending: match err.error_len() {
					Some(err_len) => &bytes[..err_len],
					None => bytes,
				},
			}),
		};

		(self.latest_index, atom)
	}

	fn consume(&mut self) -> (Self::Loc, Self::Atom) {
		// Save our current character head to `latest_index`.
		self.latest_index = self.next_index;

		// Obtain the remaining byte stream. A `head_index` of `contents.len()` produces a zero-length
		// slice.
		let remaining = &self.contents[self.next_index..];

		// If we parsed everything, return the EOF.
		if remaining.is_empty() {
			return (self.latest_index, Ok(None));
		}

		// Let's take at most 4 bytes from this stream since this is the longest codepoint we can
		// read and we don't want to validate too much text in one call to `consume`.
		let parsed = str_from_utf8(limit_len(remaining, 4));

		match parsed {
			Ok(chars) => {
				// Looks like the whole buffer was valid. Let's just get the first codepoint—which
				// we know exists because there was at least one byte in the stream.
				let char = chars.chars().next().unwrap();
				self.next_index += char.len_utf8();

				(self.latest_index, Ok(Some(char)))
			}
			Err(err) => {
				// Somewhere, the buffer became invalid. This can be for one of two reasons:
				if err.valid_up_to() == 0 {
					// Case 1: The first codepoint is not valid.
					// Let's skip it and return a codepoint parse error.
					let offending = if let Some(err_len) = err.error_len() {
						// A value of `Some` tells us that the codepoint was ended by what appears
						// to be the start of another codepoint. Let's synchronize to it and continue!
						self.next_index += err_len;

						// Everything up until the synchronization point is erroneous.
						&remaining[..err_len]
					} else {
						// A value of `None` tells us that an unexpected EOF was encountered while
						// parsing the codepoint. Let's align ourselves to the EOF!
						self.next_index = self.contents.len();

						// The entire unfinished stream is erroneous.
						remaining
					};

					(self.latest_index, Err(UnicodeParseError { offending }))
				} else {
					// Case 2: A subsequent codepoint is malformed.
					// Let's truncate it off and parse the original codepoint
					let char = str_from_utf8(&remaining[..err.valid_up_to()])
						.unwrap()
						.chars()
						.next()
						.unwrap();

					self.next_index += char.len_utf8();
					(self.latest_index, Ok(Some(char)))
				}
			}
		}
	}
}

#[derive(Debug, Copy, Clone, Error)]
#[error("failed to parse portion of unicode byte stream. Offending byte stream: {offending:?}")]
pub struct UnicodeParseError<'a> {
	offending: &'a [u8],
}

impl Atom for Result<Option<char>, UnicodeParseError<'_>> {
	fn is_eof(&self) -> bool {
		matches!(self, Ok(Some(_)))
	}
}

// === File Reader === //

/// A reader that reads logical atoms from a byte stream.
#[derive(Debug, Clone)]
pub struct FileReader<'a> {
	/// The descriptor of the file we're reading. Used to form `FileLocs`.
	file_desc: Entity,

	/// The underlying file codepoint reader.
	codepoints: CodepointReader<'a>,

	/// The last position we returned.
	latest_pos: (usize, FilePos),

	/// The next position to return.
	///
	/// The head location is different from the prev location because we want to return the starting
	/// positions of our atoms, not the ending positions. (e.g. `\n` should be the last character on
	/// a line, not the first)
	next_pos: FilePos,
}

fn fr_read_atom_untracked(codepoints: &mut CodepointReader<'_>) -> FileAtom {
	let first = match codepoints.consume_atom() {
		Ok(Some(first)) => first,
		Ok(None) => return FileAtom::Eof,
		Err(_) => return FileAtom::Malformed,
	};

	match first {
		'\n' => FileAtom::Newline {
			kind: NewlineKind::Lf,
		},
		'\r' => {
			let has_lf = codepoints
				.lookahead(|codepoints| matches!(codepoints.consume_atom(), Ok(Some('\n'))));

			let kind = if has_lf {
				NewlineKind::Crlf
			} else {
				NewlineKind::Cr
			};

			FileAtom::Newline { kind }
		}
		char => FileAtom::Codepoint(char),
	}
}

impl Cursor for FileReader<'_> {
	type Loc = FileLoc;
	type Atom = FileAtom;

	fn latest(&self) -> (Self::Loc, Self::Atom) {
		let loc = FileLoc {
			file_desc: self.file_desc,
			byte_index: self.latest_pos.0,
			pos: self.latest_pos.1,
		};

		let atom = fr_read_atom_untracked(&mut CodepointReader::new(
			&self.codepoints.contents[loc.byte_index..],
		));

		(loc, atom)
	}

	fn consume(&mut self) -> (Self::Loc, Self::Atom) {
		// Our head at the start of this routine is what we will return by the end.
		self.latest_pos = (self.codepoints.peek_loc(), self.next_pos);

		// Now, let's consume some atoms.
		let atom = fr_read_atom_untracked(&mut self.codepoints);

		// ...and update the cursor location.
		match &atom {
			FileAtom::Newline { kind: _ } => {
				self.next_pos.ln += 1;
				self.next_pos.col = 0;
			}
			_ => {
				self.next_pos.col += 1;
			}
		}

		// Finally, let's build our loc-atom pair.
		let loc = FileLoc {
			file_desc: self.file_desc,
			byte_index: self.latest_pos.0,
			pos: self.latest_pos.1,
		};
		(loc, atom)
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum FileAtom {
	Eof,
	Codepoint(char),
	Malformed,
	Newline { kind: NewlineKind },
}

impl Atom for FileAtom {
	fn is_eof(&self) -> bool {
		matches!(self, FileAtom::Eof)
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum NewlineKind {
	/// A multi-byte newline represented by a carriage return (`\r`) and a line feed (`\n`).
	/// Usually seen on Windows systems.
	Crlf,
	/// A single-byte newline represented by a line feed (`\n`). Usually seen on Unix systems.
	Lf,
	/// A mostly abandoned single-byte newline represented by a carriage return (`\r`). Usually
	/// seen on classic MacOS systems.
	Cr,
}

// === File locations === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Span {
	pub file_desc: Entity,
	pub start_byte: usize,
	pub start_pos: FilePos,
	pub end_byte: usize,
	pub end_pos: FilePos,
}

impl Span {
	pub fn new(a: FileLoc, b: FileLoc) -> Self {
		assert_eq!(a.file_desc, b.file_desc);

		let [start_loc, end_loc] = {
			let mut locs = [a, b];
			locs.sort();
			locs
		};

		Self {
			file_desc: start_loc.file_desc,
			start_byte: start_loc.byte_index,
			start_pos: start_loc.pos,
			end_byte: end_loc.byte_index,
			end_pos: end_loc.pos,
		}
	}

	pub fn start_loc(&self) -> FileLoc {
		FileLoc {
			file_desc: self.file_desc,
			byte_index: self.start_byte,
			pos: self.start_pos,
		}
	}

	pub fn end_loc(&self) -> FileLoc {
		FileLoc {
			file_desc: self.file_desc,
			byte_index: self.end_byte,
			pos: self.end_pos,
		}
	}

	pub fn byte_range(&self) -> Range<usize> {
		self.start_byte..self.end_byte
	}

	pub fn reader<'f>(&self, file: &'f LoadedFile) -> FileReader<'f> {
		assert_eq!(file.file_desc, self.file_desc);

		FileReader {
			file_desc: self.file_desc,
			codepoints: CodepointReader::new_range(&file.contents, self.byte_range()),
			latest_pos: (self.start_byte, self.start_pos),
			next_pos: self.start_pos,
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct FileLoc {
	pub file_desc: Entity,
	pub byte_index: usize,
	pub pos: FilePos,
}

impl Ord for FileLoc {
	fn cmp(&self, other: &Self) -> Ordering {
		let ord = self.byte_index.cmp(&other.byte_index);
		debug_assert_eq!(ord, self.pos.cmp(&other.pos));
		ord
	}
}

impl PartialOrd for FileLoc {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct FilePos {
	/// The zero-indexed line of the codepoint in the file.
	pub ln: usize,
	/// The zero-indexed column of the codepoint in the file.
	pub col: usize,
}

impl FilePos {
	pub const START: Self = Self { ln: 0, col: 0 };
}

impl Ord for FilePos {
	fn cmp(&self, other: &Self) -> Ordering {
		[self.ln, self.col].cmp(&[other.ln, other.col])
	}
}

impl PartialOrd for FilePos {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}
