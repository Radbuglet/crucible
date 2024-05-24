use proc_macro::TokenStream;

mod iterator;
mod transparent;

mod util;

#[proc_macro_attribute]
pub fn iterator(attrs: TokenStream, input: TokenStream) -> TokenStream {
    iterator::iterator(attrs.into(), input.into()).into()
}

#[proc_macro_attribute]
pub fn transparent(attrs: TokenStream, input: TokenStream) -> TokenStream {
    transparent::transparent(attrs.into(), input.into()).into()
}
