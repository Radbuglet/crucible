#[derive(Default)]
pub struct Emitter {
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
