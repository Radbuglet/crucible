use std::{
    fmt,
    ops::{Bound, RangeBounds},
};

use anyhow::Context;
use autoken::{cap, tie};
use crucible_utils::{
    define_index,
    mem::{defuse, guard},
    newtypes::{Index, IndexVec, LargeIndex as _},
    polyfill::OptionExt as _,
};

// === SegmentationLocale === //

// Core
pub const DEFAULT_TAB_SIZE: u32 = 4;

pub trait SegmentationLocale {
    /// Maps indexable characters in the `source` input to logical `line:column` positions in the
    /// source file.
    fn segment(&mut self, source: &str, handler: &mut impl SegmentationHandler);
}

pub trait SegmentationLocaleExt: SegmentationLocale {
    fn segment_checked(&mut self, source: &str, handler: &mut impl SegmentationHandler) {
        let mut last_pos = None;

        self.segment(source, &mut |pos, loc| {
            debug_assert!(pos < source.len());
            debug_assert!(last_pos.is_none_or(|&last_pos| pos > last_pos));
            last_pos = Some(pos);

            handler.push_pos_loc(pos, loc);
        });
    }
}

impl<T: ?Sized + SegmentationLocale> SegmentationLocaleExt for T {}

pub trait SegmentationHandler {
    fn push_pos_loc(&mut self, pos: usize, loc: FileLoc);
}

impl<F: FnMut(usize, FileLoc)> SegmentationHandler for F {
    fn push_pos_loc(&mut self, pos: usize, loc: FileLoc) {
        self(pos, loc)
    }
}

// Impls
#[derive(Debug, Clone)]
pub struct NaiveUtf8Segmenter {
    pub tab_size: u32,
}

impl Default for NaiveUtf8Segmenter {
    fn default() -> Self {
        Self {
            tab_size: DEFAULT_TAB_SIZE,
        }
    }
}

impl SegmentationLocale for NaiveUtf8Segmenter {
    fn segment(&mut self, source: &str, handler: &mut impl SegmentationHandler) {
        let mut loc = FileLoc::default();

        for (idx, char) in source.char_indices() {
            handler.push_pos_loc(idx, loc);

            if char == '\n' {
                loc.line += 1;
                loc.column = 0;
            } else if char == '\t' {
                loc.column += self.tab_size;
            } else {
                loc.column += 1;
            }
        }
    }
}

// === SpanManager === //

#[derive(Debug, Default)]
pub struct SpanManager {
    /// A buffer holding all of our sources' contents in contiguous memory. We do this instead of
    /// loading a string for each source to reduce fragmentation and make indexing easier.
    buffer: String,

    /// The names of each file.
    file_names: IndexVec<FileIndex, Box<str>>,

    /// The start indices of each file.
    file_starts: IndexVec<FileIndex, SpanPos>,

    /// Control directives sorted by `SpanPos` which indicate the line number for all characters from
    /// the current `SpanPos` (inclusive) to the `SpanPos` (exclusive) of the next tuple in this list.
    ///
    /// These control directives are emitted at every source start and new line.
    ///
    /// A newline character is considered to be part of the line it ends.
    line_ctrls: Vec<(SpanPos, u32)>,

    /// Control directives sorted by `SpanPos` which indicate the base column number of all characters
    /// from the current `SpanPos` (inclusive) to the `SpanPos` (exclusive) of the next tuple. To get
    /// the actual column number for a given `SpanPos`, we offset the column value of the tuple by
    /// the difference between our given `SpanPos` and the position value of the tuple.
    ///
    /// These control directives are emitted for each multi-byte character, grapheme cluster, tab
    /// characters, newline, and source start.
    ///
    /// A given column begins after the entire character sequence is finished.
    column_ctrls: Vec<(SpanPos, u32)>,
}

impl SpanManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(
        &mut self,
        locale: &mut impl SegmentationLocale,
        name: &str,
        load: impl FnOnce(&mut String) -> anyhow::Result<()>,
    ) -> anyhow::Result<FileIndex> {
        const ERR_CAP: &str = "loaded too many source files";

        // Write to buffer
        let start = SpanPos::try_from_usize(self.buffer.len()).context(ERR_CAP)?;
        let mut buffer_trunc_guard = guard(&mut self.buffer, |buffer| {
            buffer.truncate(start.as_usize());
        });
        load(&mut buffer_trunc_guard)?;
        let _ = SpanPos::try_from_usize(buffer_trunc_guard.len()).context(ERR_CAP)?;
        defuse(buffer_trunc_guard);

        let file = &self.buffer[start.as_usize()..];

        // Push controls
        if !file.is_empty() {
            // We always want `line_no` and `column_no` to give an actual answer, even if it's a bit
            // nonsensical, since panicking during diagnostics is arguably worse.
            self.line_ctrls.push((start, 0));
            self.column_ctrls.push((start, 0));
        }

        locale.segment_checked(file, &mut |pos, loc: FileLoc| {
            let pos = SpanPos::from_usize(start.as_usize() + pos);

            // Push control directives
            if Some(loc.line) != self.line_ctrls.last().map(|&(_, line_no)| line_no) {
                self.line_ctrls.push((pos, loc.line));
            }

            let expected_col = self
                .column_ctrls
                .last()
                .map(|&(pos_base, no_base)| no_base + pos.as_raw() - pos_base.as_raw());

            if Some(loc.column) != expected_col {
                self.column_ctrls.push((pos, loc.column));
            }
        });

        self.file_names.push(name.to_string().into_boxed_str());

        // Commit source information
        Ok(self.file_starts.push(start))
    }

    pub fn file(&self, pos: SpanPos) -> FileIndex {
        let idx = match self
            .file_starts
            .raw
            .binary_search_by(|other| other.cmp(&pos))
        {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };

        FileIndex::from_usize(idx)
    }

    pub fn file_span(&self, index: FileIndex) -> Span {
        let start = self.file_starts[index];
        let end = self
            .file_starts
            .get(index.map_usize(|v| v + 1))
            .copied()
            .unwrap_or_else(|| SpanPos::from_usize(self.buffer.len()));

        Span { start, end }
    }

    pub fn file_name(&self, file: FileIndex) -> &str {
        &self.file_names[file]
    }

    pub fn line_no(&self, pos: SpanPos) -> u32 {
        let idx = match self
            .line_ctrls
            .binary_search_by(|&(other, _)| other.cmp(&pos))
        {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };

        self.line_ctrls[idx].1
    }

    pub fn column_no(&self, pos: SpanPos) -> u32 {
        let idx = match self
            .column_ctrls
            .binary_search_by(|&(other, _)| other.cmp(&pos))
        {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };

        let (col_pos_base, col_no_base) = self.column_ctrls[idx];

        (pos.as_raw() - col_pos_base.as_raw()) + col_no_base
    }

    pub fn pos_to_loc(&self, pos: SpanPos) -> FileLoc {
        FileLoc {
            line: self.line_no(pos),
            column: self.column_no(pos),
        }
    }

    pub fn span_text(&self, span: Span) -> &str {
        &self.buffer[span.start.as_usize()..span.end.as_usize()]
    }
}

// === Span === //

define_index! {
    pub struct SpanPos: u32;
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Span {
    pub start: SpanPos,
    pub end: SpanPos,
}

impl Span {
    pub fn new(pos_a: SpanPos, pos_b: SpanPos) -> Self {
        let mut pos = [pos_a, pos_b];
        pos.sort();
        let [start, end] = pos;

        Self { start, end }
    }

    pub fn range(self, range: impl RangeBounds<usize>) -> Self {
        let start = match range.start_bound() {
            Bound::Included(i) => self.start.map_usize(|v| v + i),
            Bound::Excluded(_) => unimplemented!(),
            Bound::Unbounded => self.start,
        };

        let end = match range.end_bound() {
            Bound::Included(_) => unimplemented!(),
            Bound::Excluded(i) => {
                let end = self.start.map_usize(|v| v + i);
                assert!(end <= self.end);
                end
            }
            Bound::Unbounded => self.end,
        };

        Self { start, end }
    }

    pub fn to(self, other: Self) -> Self {
        Self::new(self.start, other.end)
    }

    pub fn between(self, other: Self) -> Self {
        Self::new(self.end, other.start)
    }

    pub fn until(self, other: Self) -> Self {
        Self::new(self.start, other.start)
    }

    pub fn fmt_with(self, manager: &SpanManager) -> SpanFormatter<'_> {
        SpanFormatter {
            manager,
            span: self,
        }
    }
}

define_index! {
    pub struct FileIndex: u32;
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct FileLoc {
    pub line: u32,
    pub column: u32,
}

// === Formatters === //

pub struct SpanFormatter<'a> {
    manager: &'a SpanManager,
    span: Span,
}

impl fmt::Debug for SpanFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}-{}",
            self.manager.file_name(self.manager.file(self.span.start)),
            self.manager.pos_to_loc(self.span.start),
            self.manager.pos_to_loc(self.span.end)
        )
    }
}

impl fmt::Display for SpanFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl fmt::Debug for FileLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line + 1, self.column + 1)
    }
}

impl fmt::Display for FileLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

// === Dependency Injection === //

cap! {
    pub SpanManagerCap = SpanManager;
}

impl SpanPos {
    pub fn file(self) -> FileIndex {
        cap!(ref SpanManagerCap).file(self)
    }

    pub fn line_no(self) -> u32 {
        cap!(ref SpanManagerCap).line_no(self)
    }

    pub fn column_no(self) -> u32 {
        cap!(ref SpanManagerCap).column_no(self)
    }

    pub fn to_loc(self) -> FileLoc {
        cap!(ref SpanManagerCap).pos_to_loc(self)
    }
}

impl Span {
    pub fn file(self) -> FileIndex {
        self.start.file()
    }

    pub fn text<'a>(self) -> &'a str {
        tie!('a => ref SpanManagerCap);
        cap!(ref SpanManagerCap).span_text(self)
    }

    pub fn fmt<'a>(self) -> SpanFormatter<'a> {
        tie!('a => ref SpanManagerCap);
        self.fmt_with(cap!(ref SpanManagerCap))
    }
}

impl FileIndex {
    pub fn new(
        locale: &mut impl SegmentationLocale,
        name: &str,
        load: impl FnOnce(&mut String) -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
        cap!(mut SpanManagerCap).load(locale, name, load)
    }

    pub fn span(self) -> Span {
        cap!(ref SpanManagerCap).file_span(self)
    }

    pub fn name<'a>(self) -> &'a str {
        tie!('a => ref SpanManagerCap);
        cap!(ref SpanManagerCap).file_name(self)
    }
}
