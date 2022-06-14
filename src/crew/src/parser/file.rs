use crate::parser::generic::Cursor;
use crate::util::iter_magic::limit_len;
use crate::util::obj::Entity;
use std::cmp::Ordering;
use std::str::from_utf8 as str_from_utf8;

// === Source file representations === //

/// Optional file descriptor metadata to help users find the corresponding source file.
#[derive(Debug, Clone)]
pub struct SourceFileInfo {
	pub name: String,
}

/// A file whose contents have been loaded into memory.
#[derive(Debug, Clone)]
pub struct LoadedFile {
	pub file_desc: Entity,
	pub contents: Vec<u8>,
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
	pub fn new(contents: &'a [u8], ) -> Self {

	}
}

impl<'a> Cursor for CodepointReader<'a> {
	type Atom = Result<Option<char>, UnicodeParseError<'a>>;

	fn latest(&self) -> (Self::Loc, Self::Atom) {
		if self.latest_index == self.next_index {
			return (self.latest_index, Ok(None));
		}

		let bytes = &self.contents[self.latest_index..self.next_index];
		let atom = match str_from_utf8(bytes) {
			Ok(str) => Ok(str.chars().next()),
			Err(err) => Err(UnicodeParseError {
				bytes: match err.error_len() {
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
					let relevant_bytes = if let Some(err_len) = err.error_len() {
						// A value of `Some` tells us that the codepoint was ended by what appears
						// to be the start of another codepoint. Let's synchronize to it and continue!
						self.next_index += err_len;

						// Everything up until the synchronization point is errouneous.
						&remaining[..err_len]
					} else {
						// A value of `None` tells us that an unexpected EOF was encountered while
						// parsing the codepoint. Let's align ourselves to the EOF!
						self.next_index = self.contents.len();

						// The entire unfinished stream is errouneous.
						remaining
					};

					(
						self.latest_index,
						Err(UnicodeParseError {
							bytes: relevant_bytes,
						}),
					)
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
	next_pos: FilePos,
}

impl Cursor for FileReader<'_> {
	type Loc = FileLoc;
	type Atom = FileAtom;

	fn latest(&self) -> (Self::Loc, Self::Atom) {
		todo!()
	}

	fn consume(&mut self) -> (Self::Loc, Self::Atom) {
		// Our latest position becomes our next position
		self.latest_pos = self.next_pos;

		let (first_loc, first_char) = self.codepoints.consume();
		let first_char = match first_char {
			Ok(Some(char)) => char,
			Ok(None) => ()
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum FileAtom {
	Eof,
	Codepoint(char),
	Malformed,
	Newline { kind: NewlineKind },
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum NewlineKind {
	/// A multi-byte newline represented by a carriage return (`\r`) and a line feed (`\n`).
	/// Usually seen on Windows systems.
	Crlf,
	/// A single-byte newline represented by a line feed (`\n`). Usually seen on Unix systems.
	Lf,
	/// A mostly abandonned single-byte newline represented by a carriage return (`\r`). Usually
	/// seen on classic MacOS systems.
	Cr,
}

// === File locations === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Span {
	pub file_desc: Entity,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct FileLoc {
	pub file_desc: Entity,
	pub index: usize,
	pub pos: FilePos,
}

impl Ord for FileLoc {
	fn cmp(&self, other: &Self) -> Ordering {
		let ord = self.index.cmp(&other.index);
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
	pub ln: usize,
	pub col: usize,
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
