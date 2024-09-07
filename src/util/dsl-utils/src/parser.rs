use std::{
    cell::{Cell, RefCell},
    fmt,
    thread::panicking,
};

use crucible_utils::{hash::FxHashSet, newtypes::Index as _};

use crate::{
    diagnostic::{report_diagnostic, Diagnostic},
    span::{Span, SpanFile, SpanManagerCap, SpanPos},
    symbol::Symbol,
};

// === LookaheadResult === //

pub trait LookaheadResult {
    fn is_truthy(&self) -> bool;
}

impl LookaheadResult for bool {
    fn is_truthy(&self) -> bool {
        *self
    }
}

impl<T> LookaheadResult for Option<T> {
    fn is_truthy(&self) -> bool {
        self.is_some()
    }
}

impl<T, E> LookaheadResult for Result<T, E> {
    fn is_truthy(&self) -> bool {
        self.is_ok()
    }
}

// === ParseContext === //

#[derive(Debug, Clone, Default)]
pub struct ParseContext {
    while_parsing: RefCell<Vec<(Span, Symbol)>>,
    got_stuck: Cell<bool>,
}

impl ParseContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enter<I>(&self, cursor: I) -> ParseSequence<'_, I> {
        ParseSequence {
            cx: self,
            cursor,
            expectations: Vec::new(),
            stuck_hints: Vec::new(),
        }
    }

    pub fn while_parsing(&self, starting_at: Span, what: Symbol) -> WhileParsingGuard<'_> {
        self.while_parsing.borrow_mut().push((starting_at, what));

        WhileParsingGuard {
            cx: self,
            top: what,
        }
    }

    pub fn got_stuck(&self) -> bool {
        self.got_stuck.get()
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct WhileParsingGuard<'c> {
    cx: &'c ParseContext,
    top: Symbol,
}

impl Drop for WhileParsingGuard<'_> {
    fn drop(&mut self) {
        let popped = self.cx.while_parsing.borrow_mut().pop();
        if !panicking() {
            assert!(matches!(popped, Some((_, v)) if v == self.top));
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseSequence<'cx, I> {
    cx: &'cx ParseContext,
    cursor: I,
    expectations: Vec<Symbol>,
    stuck_hints: Vec<(Span, String)>,
}

impl<'cx, I> ParseSequence<'cx, I> {
    pub fn enter<I2>(&self, cursor: I2) -> ParseSequence<'cx, I2> {
        self.cx.enter(cursor)
    }

    pub fn while_parsing(&self, what: Symbol) -> WhileParsingGuard<'cx>
    where
        I: ParseCursor,
    {
        self.cx.while_parsing(self.next_span(), what)
    }

    fn moved_forward(&mut self) {
        self.expectations.clear();
        self.stuck_hints.clear();
    }

    pub fn irrefutable<R>(&mut self, f: impl FnOnce(&mut I) -> R) -> R {
        self.moved_forward();
        f(&mut self.cursor)
    }

    pub fn expect_covert_hinted<R>(
        &mut self,
        visible: bool,
        expectation: Symbol,
        f: impl FnOnce(&mut I, &mut ParseHinter<'_>) -> R,
    ) -> R
    where
        I: ParseCursor,
        R: LookaheadResult,
    {
        let mut hinter = ParseHinter {
            hints: Some(&mut self.stuck_hints),
        };
        let res = self.cursor.lookahead(|c| f(c, &mut hinter));
        if res.is_truthy() {
            self.moved_forward();
        } else if visible {
            self.expectations.push(expectation);
        }
        res
    }

    pub fn expect_covert<R>(
        &mut self,
        visible: bool,
        expectation: Symbol,
        f: impl FnOnce(&mut I) -> R,
    ) -> R
    where
        I: ParseCursor,
        R: LookaheadResult,
    {
        self.expect_covert_hinted(visible, expectation, |c, _| f(c))
    }

    pub fn expect<R>(&mut self, expectation: Symbol, f: impl FnOnce(&mut I) -> R) -> R
    where
        I: ParseCursor,
        R: LookaheadResult,
    {
        self.expect_covert(true, expectation, |c| f(c))
    }

    pub fn expect_hinted<R>(
        &mut self,
        expectation: Symbol,
        f: impl FnOnce(&mut I, &mut ParseHinter<'_>) -> R,
    ) -> R
    where
        I: ParseCursor,
        R: LookaheadResult,
    {
        self.expect_covert_hinted(true, expectation, f)
    }

    pub fn hint_stuck_if_passes<R>(
        &mut self,
        reason: impl fmt::Display,
        f: impl FnOnce(&mut I) -> R,
    ) where
        I: ParseCursor,
        R: LookaheadResult,
    {
        let start = self.next_span();
        let mut fork = self.cursor.clone();
        if f(&mut fork).is_truthy() {
            self.hint_stuck(start.until(fork.next_span()), reason.to_string());
        }
    }

    pub fn hint_stuck(&mut self, span: Span, reason: impl Into<String>) {
        self.stuck_hints.push((span, reason.into()));
    }

    pub fn stuck_lookahead<R>(&mut self, recover: impl FnOnce(&mut I) -> R)
    where
        I: ParseCursor,
        R: LookaheadResult,
    {
        self.stuck(|c| {
            c.lookahead(recover);
        });
    }

    pub fn stuck(&mut self, recover: impl FnOnce(&mut I))
    where
        I: ParseCursor,
    {
        // Mark that we got stuck
        self.cx.got_stuck.set(true);

        // Emit the error message
        let span = self.cursor.next_span();

        let expectations = self.expectations.iter().copied().collect::<FxHashSet<_>>();
        let mut expectations = expectations
            .iter()
            .map(|v| v.as_string())
            .collect::<Vec<_>>();

        expectations.sort_unstable();

        if expectations.is_empty() {
            expectations.push("<nothing?>".to_string());
        }

        if let Some(last) = (expectations.len() > 1)
            .then(|| expectations.last_mut())
            .flatten()
        {
            last.insert_str(0, "or ");
        }
        let expectations = expectations.join(", ");

        let while_parsing = {
            let stack = self.cx.while_parsing.borrow();
            if stack.is_empty() {
                String::new()
            } else {
                format!(
                    " while parsing {}",
                    stack
                        .iter()
                        .rev()
                        .map(|(loc, what)| format!("{} starting at {}", what.as_str(), loc.fmt()))
                        .collect::<Vec<_>>()
                        .join(" in ")
                )
            }
        };

        report_diagnostic({
            let mut diagnostic =
                Diagnostic::span_err(span, format!("expected {expectations}{while_parsing}"));

            for (hint_span, hint) in &self.stuck_hints {
                diagnostic
                    .subs
                    .push(Diagnostic::span_note(*hint_span, hint.clone()));
            }
            diagnostic
        });

        // Attempt to get unstuck
        recover(&mut self.cursor);

        // Clear expectation set
        self.moved_forward();
    }

    pub fn error(&mut self, diagnostic: impl Into<Diagnostic>, recover: impl FnOnce(&mut I)) {
        self.cx.got_stuck.set(true);
        report_diagnostic(diagnostic.into());
        recover(&mut self.cursor);
    }

    pub fn next_span(&self) -> Span
    where
        I: ParseCursor,
    {
        self.cursor.next_span()
    }

    pub fn prev_span(&self) -> Span
    where
        I: LookBackParseCursor,
    {
        self.cursor.prev_span()
    }

    pub fn context(&self) -> &'cx ParseContext {
        self.cx
    }
}

#[derive(Debug)]
pub struct ParseHinter<'a> {
    hints: Option<&'a mut Vec<(Span, String)>>,
}

impl ParseHinter<'_> {
    pub fn new_empty() -> Self {
        Self { hints: None }
    }

    pub fn hint(&mut self, span: Span, message: impl fmt::Display) {
        if let Some(hints) = self.hints.as_deref_mut() {
            hints.push((span, message.to_string()));
        }
    }
}

// === ParseOption === //

pub trait OptionParser<I>: Sized {
    type Handler: Fn(&mut I, &mut ParseHinter<'_>) -> Self::Output;
    type Output: LookaheadResult;

    fn expectation(&self) -> Symbol;

    fn matcher(&self) -> &Self::Handler;

    fn consume(&self, c: &mut I) -> Self::Output
    where
        I: ForkableCursor,
    {
        c.lookahead(|c| self.matcher()(c, &mut ParseHinter::new_empty()))
    }

    fn did_consume(&self, c: &mut I) -> bool
    where
        I: ForkableCursor,
    {
        self.consume(c).is_truthy()
    }

    fn expect(&self, c: &mut ParseSequence<'_, I>) -> Self::Output
    where
        I: ParseCursor,
    {
        c.expect_hinted(self.expectation(), self.matcher())
    }

    fn expect_covert(&self, visible: bool, c: &mut ParseSequence<'_, I>) -> Self::Output
    where
        I: ParseCursor,
    {
        c.expect_covert_hinted(visible, self.expectation(), self.matcher())
    }

    fn hint_if_match(&self, c: &mut ParseSequence<'_, I>, reason: impl fmt::Display)
    where
        I: ParseCursor,
    {
        c.hint_stuck_if_passes(reason, |c| self.matcher()(c, &mut ParseHinter::new_empty()))
    }
}

impl<C, F, O> OptionParser<C> for (Symbol, F)
where
    F: Fn(&mut C, &mut ParseHinter<'_>) -> O,
    O: LookaheadResult,
{
    type Handler = F;
    type Output = O;

    fn expectation(&self) -> Symbol {
        self.0
    }

    fn matcher(&self) -> &Self::Handler {
        &self.1
    }
}

// === Cursors === //

// Core
pub trait ForkableCursor: Iterator + Clone {
    fn peek(&self) -> Option<Self::Item> {
        self.clone().next()
    }

    fn lookahead<R: LookaheadResult>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let mut fork = self.clone();
        let res = f(&mut fork);
        if res.is_truthy() {
            *self = fork;
        }

        res
    }
}

pub trait ParseCursor: ForkableCursor {
    fn next_span(&self) -> Span;
}

pub trait LookBackParseCursor: ParseCursor {
    fn prev_span(&self) -> Span;
}

// StrCursor
pub type StrSequence<'a> = ParseSequence<'a, StrCursor<'a>>;

#[derive(Debug, Clone)]
pub struct StrCursor<'a> {
    full_span: Span,
    prev_span: Span,
    iter: std::str::CharIndices<'a>,
}

impl<'a> StrCursor<'a> {
    pub fn new(start_pos: SpanPos, text: &'a str) -> Self {
        Self {
            full_span: Span::new(start_pos, start_pos.map_usize(|v| v + text.len())),
            prev_span: Span::new(start_pos, start_pos),
            iter: text.char_indices(),
        }
    }

    pub fn new_span(span: Span) -> Self {
        autoken::tie!('a => ref SpanManagerCap);
        Self::new(span.start, span.text())
    }

    pub fn new_file(file: SpanFile) -> Self {
        autoken::tie!('a => ref SpanManagerCap);
        Self::new_span(file.span())
    }
}

impl Iterator for StrCursor<'_> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, c)| c)
    }
}

impl ForkableCursor for StrCursor<'_> {}

impl ParseCursor for StrCursor<'_> {
    fn next_span(&self) -> Span {
        let mut iter = self.iter.clone();

        let start = iter
            .next()
            .map(|(i, _)| self.full_span.start.map_usize(|b| b + i))
            .unwrap_or(self.full_span.end);

        let end = iter
            .next()
            .map(|(i, _)| self.full_span.start.map_usize(|b| b + i))
            .unwrap_or(self.full_span.end);

        Span { start, end }
    }
}

impl LookBackParseCursor for StrCursor<'_> {
    fn prev_span(&self) -> Span {
        self.prev_span
    }
}

// === MaybePlaceholder === //

pub type OptMaybePlaceholder<T> = Option<MaybePlaceholder<T>>;

pub type MaybePlaceholder<T> = Result<T, CannotConstruct>;

#[derive(Debug, Copy, Clone, Default)]
pub struct CannotConstruct;
