use proc_macro2::{Group, Span, TokenStream, TokenTree};

use crate::util::Emitter;

pub fn copy_hygiene(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::default();

    let mut input = input.clone().into_iter();

    let Ok(first) = input.next().ok_or(()).map_err(|_| {
        emitter.err(syn::Error::new(
            Span::call_site(),
            "expected template from first token",
        ));
    }) else {
        return emitter.finish();
    };

    if input
        .next()
        .filter(|tt| matches!(tt, TokenTree::Punct(p) if p.as_char() == ','))
        .ok_or(())
        .map_err(|_| {
            emitter.err(syn::Error::new(
                Span::call_site(),
                "missing `,` after span copy token",
            ))
        })
        .is_err()
    {
        return emitter.finish();
    };

    copy_hygiene_inner(first.span(), input)
}

fn copy_hygiene_inner(span: Span, input: impl IntoIterator<Item = TokenTree>) -> TokenStream {
    input
        .into_iter()
        .map(|mut v| {
            match &mut v {
                TokenTree::Group(v) => {
                    let mut v_new = Group::new(v.delimiter(), copy_hygiene_inner(span, v.stream()));
                    v_new.set_span(span);
                    *v = v_new
                }
                TokenTree::Ident(v) => {
                    v.set_span(span);
                }
                TokenTree::Punct(v) => {
                    v.set_span(span);
                }
                TokenTree::Literal(v) => {
                    v.set_span(span);
                }
            }
            v
        })
        .collect()
}
