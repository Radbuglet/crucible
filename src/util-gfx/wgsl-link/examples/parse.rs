use lang_utils::{
    diagnostic::{has_diagnostic_errors, DiagnosticReporter},
    span::{NaiveUtf8Segmenter, SpanManager},
    symbol::Interner,
};
use wgsl_link::driver::parser::parse_directives;

fn main() {
    let mut interner = Interner::new();
    let mut span_mgr = SpanManager::new();
    let mut diagnostics = DiagnosticReporter::new();

    let file = span_mgr
        .load(&mut NaiveUtf8Segmenter::default(), "c.wgsl", |v| {
            v.push_str(include_str!("c.wgsl"));
            Ok(())
        })
        .unwrap();

    parse_directives(
        &mut diagnostics,
        &mut interner,
        &span_mgr,
        span_mgr.file_span(file),
    );

    if has_diagnostic_errors() {
        dbg!(&diagnostics);
    }
}
