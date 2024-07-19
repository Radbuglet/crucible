use anyhow::Context;
use crucible_utils::{
    define_index,
    lifetimes::{defuse, guard},
    newtypes::{Index, IndexVec},
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
    fn push_pos_loc(&mut self, pos: usize, loc: SourceLoc);
}

impl<F: FnMut(usize, SourceLoc)> SegmentationHandler for F {
    fn push_pos_loc(&mut self, pos: usize, loc: SourceLoc) {
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
        let mut loc = SourceLoc::default();

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

    /// The start indices of each source.
    source_starts: IndexVec<SourceIndex, SpanPos>,

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
        load: impl FnOnce(&mut String) -> anyhow::Result<()>,
    ) -> anyhow::Result<Span> {
        const ERR_CAP: &str = "loaded too many sources";

        // Write to buffer
        let start = SpanPos::try_from_usize(self.buffer.len()).context(ERR_CAP)?;
        let mut buffer_trunc_guard = guard(&mut self.buffer, |buffer| {
            buffer.truncate(start.as_usize());
        });
        load(&mut buffer_trunc_guard)?;
        let end = SpanPos::try_from_usize(buffer_trunc_guard.len()).context(ERR_CAP)?;
        defuse(buffer_trunc_guard);

        let source = &self.buffer[start.as_usize()..];

        // Push controls
        if !source.is_empty() {
            // We always want `line_no` and `column_no` to give an actual answer, even if it's a bit
            // nonsensical, since panicking during diagnostics is arguably worse.
            self.line_ctrls.push((start, 0));
            self.column_ctrls.push((start, 0));
        }

        locale.segment_checked(source, &mut |pos, loc: SourceLoc| {
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

        // Commit source information
        self.source_starts.push(start);

        Ok(Span { start, end })
    }

    pub fn source_idx(&self, pos: SpanPos) -> SourceIndex {
        let idx = match self
            .source_starts
            .raw
            .binary_search_by(|other| pos.cmp(other))
        {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };

        SourceIndex::from_usize(idx)
    }

    pub fn source_span(&self, index: SourceIndex) -> Span {
        let start = self.source_starts[index];
        let end = self
            .source_starts
            .get(index)
            .copied()
            .unwrap_or_else(|| SpanPos::from_usize(self.buffer.len()));

        Span { start, end }
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

    pub fn pos_to_loc(&self, pos: SpanPos) -> SourceLoc {
        SourceLoc {
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

// === Source === //

define_index! {
    pub struct SourceIndex: u32;
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct SourceLoc {
    pub line: u32,
    pub column: u32,
}

// === Dependency Injection === //

// TODO
