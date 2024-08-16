use autoken::cap;
use lang_utils::{
    define_keywords,
    diagnostic::{DiagnosticReporter, DiagnosticReporterCap},
    parser::{OptionParser, ParseContext},
    punct,
    span::{Span, SpanManager, SpanManagerCap},
    symbol::{Interner, InternerCap, Symbol},
    tokens::tokenize,
    tokens_parse::{eof, identifier, keyword, punct, str_lit},
};

define_keywords! {
    enum DirectiveKeyword {
        As = "as",
        In = "in",
        Use = "use",
    }
}

pub fn parse_directives(
    diagnostics: &mut DiagnosticReporter,
    interns: &mut Interner,
    spans: &SpanManager,
    source: Span,
) {
    cap! {
        DiagnosticReporterCap: diagnostics,
        InternerCap: interns,
        SpanManagerCap: spans,
    =>
        parse_directives_inner(source);
    }
}

fn parse_directives_inner(source: Span) {
    let source_txt = source.text();

    // Handle directives on the first line
    if source_txt.starts_with("//#") {
        // Skip past `//#`
        let start = "//#".len();

        // End directive span at line end
        let mut end = start
            + memchr::memchr(b'\n', source_txt[start..].as_bytes()).unwrap_or(source_txt.len());

        if source_txt.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }

        // Parse the directive
        parse_directive(source.range(start..end));
    }

    // Handle directives on subsequent lines
    for start in memchr::memmem::find_iter(source_txt.as_bytes(), b"\n//#") {
        // Skip past `\n//#`
        let start = start + b"\n//#".len();

        // End directive span at line end
        let mut end = start
            + memchr::memchr(b'\n', source_txt[start..].as_bytes()).unwrap_or(source_txt.len());

        if source_txt.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }

        // Parse the directive
        parse_directive(source.range(start..end));
    }
}

fn parse_directive(span: Span) {
    let tokens = tokenize(span);

    let cx = ParseContext::new();
    let mut p = cx.enter(tokens.cursor());

    let directive_start = p.next_span();

    if keyword(DirectiveKeyword::Use).expect(&mut p).is_none() {
        p.stuck(|_| ());
        return;
    }

    let _pg1 = p
        .context()
        .while_parsing(directive_start, Symbol::new_static("`use` directive"));

    let mut names = Vec::new();

    loop {
        let Some(name) =
            identifier::<DirectiveKeyword>(Symbol::new_static("<imported symbol name>"))
                .expect(&mut p)
        else {
            p.stuck(|_| ());
            return;
        };

        let rename = if keyword(DirectiveKeyword::As).expect(&mut p).is_some() {
            let rename =
                identifier::<DirectiveKeyword>(Symbol::new_static("<renamed symbol name>"))
                    .expect(&mut p);

            if rename.is_none() {
                p.stuck(|_| ());
            }

            rename
        } else {
            None
        };

        names.push((name, rename));

        if punct(punct!(',')).expect(&mut p).is_none() {
            break;
        }
    }

    if keyword(DirectiveKeyword::In).expect(&mut p).is_none() {
        p.stuck(|_| ());
        return;
    }

    let Some(file) = str_lit(Symbol::new_static("<file path>")).expect(&mut p) else {
        p.stuck(|_| ());
        return;
    };

    if !eof(Symbol::new_static("newline")).expect(&mut p) {
        p.stuck(|_| ());
    }

    dbg!(
        names
            .iter()
            .map(|(a, b)| (a.text.as_str(), b.map(|b| b.text.as_str())))
            .collect::<Vec<_>>(),
        file.inner.as_str()
    );
}
