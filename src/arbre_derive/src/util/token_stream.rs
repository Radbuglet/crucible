use proc_macro2::{Group, Span, TokenStream, TokenTree};

pub trait MyTokenStreamExt: Sized {
    fn with_span(self, span: Span) -> Self;
}

impl MyTokenStreamExt for TokenStream {
    fn with_span(self, span: Span) -> Self {
        self.into_iter()
            .map(move |mut token| match token {
                TokenTree::Group(group) => TokenTree::Group(Group::new(
                    group.delimiter(),
                    group.stream().with_span(span),
                )),
                _ => {
                    token.set_span(span);
                    token
                }
            })
            .collect()
    }
}
