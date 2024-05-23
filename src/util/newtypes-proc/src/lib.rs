use proc_macro::TokenStream;

mod iterator;
mod transparent;

#[derive(Default)]
struct Emitter {
    tokens: proc_macro2::TokenStream,
}

impl Emitter {
    pub fn push(&mut self, tokens: impl quote::ToTokens) {
        tokens.to_tokens(&mut self.tokens);
    }

    pub fn err(&mut self, err: syn::Error) {
        self.push(err.into_compile_error());
    }

    pub fn finish(self) -> proc_macro2::TokenStream {
        self.tokens
    }
}

#[proc_macro_attribute]
pub fn iterator(attrs: TokenStream, input: TokenStream) -> TokenStream {
    iterator::iterator(attrs.into(), input.into()).into()
}

#[proc_macro_attribute]
pub fn transparent(attrs: TokenStream, input: TokenStream) -> TokenStream {
    transparent::transparent(attrs.into(), input.into()).into()
}
