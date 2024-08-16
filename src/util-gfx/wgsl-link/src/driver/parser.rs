use autoken::cap;
use lang_utils::{
    diagnostic::{DiagnosticReporter, DiagnosticReporterCap},
    parser::ParseContext,
    punct,
    span::{Span, SpanManager, SpanManagerCap},
    symbol::{Interner, InternerCap, Symbol},
    tokens::{tokenize, Token},
};

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

    if !p.expect(Symbol::new_static("`use`"), |p| {
        p.next()
            .is_some_and(|v| matches!(v, Token::Ident(i) if i.text == Symbol::new_static("use")))
    }) {
        p.stuck(|_| ());
        return;
    }

    let _pg1 = p
        .context()
        .while_parsing(directive_start, Symbol::new_static("`use` directive"));

    let mut names = Vec::new();

    loop {
        let Some(name) = p.expect(Symbol::new_static("<imported symbol name>"), |p| {
            p.next().and_then(|v| v.as_ident())
        }) else {
            p.stuck(|_| ());
            return;
        };

        let rename = if p.expect(Symbol::new_static("`as`"), |p| {
            p.next()
                .is_some_and(|v| matches!(v, Token::Ident(i) if i.text == Symbol::new_static("as")))
        }) {
            let rename = p.expect(Symbol::new_static("<renamed symbol name>"), |p| {
                p.next().and_then(|v| v.as_ident())
            });

            if rename.is_none() {
                p.stuck(|_| ());
            }

            rename
        } else {
            None
        };

        names.push((name, rename));

        if !p.expect(Symbol::new_static("`,`"), |p| {
            p.next()
                .and_then(|c| c.as_punct())
                .is_some_and(|c| c.char == punct!(','))
        }) {
            break;
        }
    }

    if !p.expect(Symbol::new_static("`in`"), |c| {
        c.next()
            .is_some_and(|v| matches!(v, Token::Ident(i) if i.text == Symbol::new_static("in")))
    }) {
        p.stuck(|_| ());
        return;
    }

    let Some(file) = p.expect(Symbol::new_static("<file path>"), |c| {
        c.next().and_then(|t| t.as_string_lit())
    }) else {
        p.stuck(|_| ());
        return;
    };

    if !p.expect(Symbol::new_static("newline"), |c| c.next().is_none()) {
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
