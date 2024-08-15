use autoken::cap;
use lang_utils::{
    diagnostic::{has_diagnostic_errors, DiagnosticReporter, DiagnosticReporterCap},
    span::{FileIndex, NaiveUtf8Segmenter, SpanManager, SpanManagerCap},
    symbol::{Interner, InternerCap},
    tokens::tokenize,
};

fn main() {
    let mut interner = Interner::new();
    let mut span_mgr = SpanManager::new();
    let mut diagnostics = DiagnosticReporter::new();

    cap! {
        InternerCap: &mut interner,
        SpanManagerCap: &mut span_mgr,
        DiagnosticReporterCap: &mut diagnostics,
    =>
        let file = FileIndex::new(&mut NaiveUtf8Segmenter::default(), |v| {
            v.push_str("foo bar_baz\n naz hello { é whee + 3i32 'h'}");
            Ok(())
        }).unwrap();

        let tokens = tokenize(file);

        if has_diagnostic_errors() {
            dbg!(cap!(ref DiagnosticReporterCap));
        } else {
            dbg!(tokens);
        }
    }
}
