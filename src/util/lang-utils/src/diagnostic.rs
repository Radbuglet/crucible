use std::io;

use autoken::cap;
use codespan_reporting::term::{self, termcolor::WriteColor};
use crucible_utils::newtypes::Index;

use crate::span::{Span, SpanManager};

// === DiagnosticReporter === //

#[derive(Debug, Default)]
pub struct DiagnosticReporter {
    diagnostics: Vec<Diagnostic>,
    has_errors: bool,
}

impl DiagnosticReporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn report(&mut self, diagnostic: Diagnostic) {
        if diagnostic.kind == DiagnosticKind::Error {
            self.has_errors = true;
        }

        self.diagnostics.push(diagnostic);
    }

    pub fn has_errors(&self) -> bool {
        self.has_errors
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

// === Diagnostic === //

pub const NO_CODE: u32 = u32::MAX;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub code: u32,
    pub message: String,
    pub offending_span: Option<Span>,
    pub windows: Vec<DiagnosticWindow>,
    pub subs: Vec<Diagnostic>,
}

impl Diagnostic {
    // === Constructors === //

    pub fn new(kind: DiagnosticKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            code: NO_CODE,
            message: message.into(),
            offending_span: None,
            windows: Vec::new(),
            subs: Vec::new(),
        }
    }

    // Span
    pub fn span_bug(span: Span, message: impl Into<String>) -> Self {
        Self::new_bug(message).with_offending_span(span)
    }

    pub fn span_err(span: Span, message: impl Into<String>) -> Self {
        Self::new_err(message).with_offending_span(span)
    }

    pub fn span_warn(span: Span, message: impl Into<String>) -> Self {
        Self::new_warn(message).with_offending_span(span)
    }

    pub fn span_info(span: Span, message: impl Into<String>) -> Self {
        Self::new_info(message).with_offending_span(span)
    }

    pub fn span_note(span: Span, message: impl Into<String>) -> Self {
        Self::new_note(message).with_offending_span(span)
    }

    pub fn span_help(span: Span, message: impl Into<String>) -> Self {
        Self::new_help(message).with_offending_span(span)
    }

    pub fn opt_span_bug(span: Option<Span>, message: impl Into<String>) -> Self {
        Self::new_bug(message).with_opt_offending_span(span)
    }

    pub fn opt_span_err(span: Option<Span>, message: impl Into<String>) -> Self {
        Self::new_err(message).with_opt_offending_span(span)
    }

    pub fn opt_span_warn(span: Option<Span>, message: impl Into<String>) -> Self {
        Self::new_warn(message).with_opt_offending_span(span)
    }

    pub fn opt_span_info(span: Option<Span>, message: impl Into<String>) -> Self {
        Self::new_info(message).with_opt_offending_span(span)
    }

    pub fn opt_span_note(span: Option<Span>, message: impl Into<String>) -> Self {
        Self::new_note(message).with_opt_offending_span(span)
    }

    pub fn opt_span_help(span: Option<Span>, message: impl Into<String>) -> Self {
        Self::new_help(message).with_opt_offending_span(span)
    }

    // Un-spanned
    pub fn new_bug(message: impl Into<String>) -> Self {
        Self::new(DiagnosticKind::Bug, message)
    }

    pub fn new_err(message: impl Into<String>) -> Self {
        Self::new(DiagnosticKind::Error, message)
    }

    pub fn new_warn(message: impl Into<String>) -> Self {
        Self::new(DiagnosticKind::Warn, message)
    }

    pub fn new_info(message: impl Into<String>) -> Self {
        Self::new(DiagnosticKind::Info, message)
    }

    pub fn new_note(message: impl Into<String>) -> Self {
        Self::new(DiagnosticKind::Note, message)
    }

    pub fn new_help(message: impl Into<String>) -> Self {
        Self::new(DiagnosticKind::Help, message)
    }

    // === Builder === //

    pub fn with_offending_span(mut self, span: Span) -> Self {
        self.offending_span = Some(span);
        self
    }

    pub fn with_opt_offending_span(mut self, span: Option<Span>) -> Self {
        self.offending_span = span;
        self
    }

    pub fn with_code(mut self, code: u32) -> Self {
        self.code = code;
        self
    }

    pub fn with_window(mut self, span: Span, label: Option<impl Into<String>>) -> Self {
        self.windows.push(DiagnosticWindow {
            span,
            label: label.map(Into::into),
        });
        self
    }

    pub fn with_sub(mut self, sub: Diagnostic) -> Self {
        self.subs.push(sub);
        self
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum DiagnosticKind {
    Bug,
    Error,
    Warn,
    Info,
    Note,
    Help,
}

#[derive(Debug, Clone)]
pub struct DiagnosticWindow {
    pub span: Span,
    pub label: Option<String>,
}

// === Dependency Injection === //

cap! {
    pub DiagnosticReporterCap = DiagnosticReporter;
}

pub fn report_diagnostic(diagnostic: Diagnostic) {
    cap!(mut DiagnosticReporterCap).report(diagnostic);
}

pub fn has_diagnostic_errors() -> bool {
    cap!(ref DiagnosticReporterCap).has_errors()
}

// === Printing === //

pub fn emit_pretty_diagnostics(
    writer: &mut dyn WriteColor,
    spans: &SpanManager,
    diags: &DiagnosticReporter,
) -> io::Result<()> {
    use codespan_reporting::{
        diagnostic::{Diagnostic, Label, LabelStyle, Severity},
        files::{Error, SimpleFiles},
        term::Config,
    };

    let mut files = SimpleFiles::new();

    for file in spans.file_indices() {
        files.add(spans.file_name(file), spans.file_text(file));
    }

    for diag in diags.diagnostics() {
        let new_diag = Diagnostic::new(match diag.kind {
            DiagnosticKind::Bug => Severity::Bug,
            DiagnosticKind::Error => Severity::Error,
            DiagnosticKind::Warn => Severity::Warning,
            DiagnosticKind::Info => unimplemented!("root diagnostic cannot have kind `info`"),
            DiagnosticKind::Note => Severity::Note,
            DiagnosticKind::Help => unimplemented!("root diagnostic cannot have kind `help`"),
        })
        .with_message(diag.message.clone())
        .with_labels(
            diag.windows
                .iter()
                .map(|window| {
                    let (file, span) = spans.span_to_range(window.span);
                    Label::new(LabelStyle::Secondary, file.as_usize(), span)
                })
                .chain(diag.offending_span.map(|span| {
                    let (file, span) = spans.span_to_range(span);
                    Label::new(LabelStyle::Primary, file.as_usize(), span)
                }))
                .collect(),
        )
        .with_notes(diag.subs.iter().map(|diag| diag.message.clone()).collect());

        let new_diag = if diag.code != u32::MAX {
            new_diag.with_code(format!("E{}", diag.code))
        } else {
            new_diag
        };

        if let Err(err) = term::emit(writer, &Config::default(), &files, &new_diag) {
            match err {
                Error::Io(err) => return Err(err),
                err => panic!("{err}"),
            }
        }
    }

    Ok(())
}
