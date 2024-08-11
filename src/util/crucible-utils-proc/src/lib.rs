use proc_macro::TokenStream;

mod copy_hygiene;
mod delegate;
mod iterator;
mod multi_closure;
mod transparent;

mod util;

#[proc_macro]
pub fn copy_hygiene(input: TokenStream) -> TokenStream {
    copy_hygiene::copy_hygiene(input.into()).into()
}

#[proc_macro]
pub fn delegate(input: TokenStream) -> TokenStream {
    delegate::delegate(input.into()).into()
}

#[proc_macro_attribute]
pub fn iterator(attrs: TokenStream, input: TokenStream) -> TokenStream {
    iterator::iterator(attrs.into(), input.into()).into()
}

#[proc_macro]
pub fn multi_closure(input: TokenStream) -> TokenStream {
    multi_closure::multi_closure(input.into()).into()
}

#[proc_macro_attribute]
pub fn transparent(attrs: TokenStream, input: TokenStream) -> TokenStream {
    transparent::transparent(attrs.into(), input.into()).into()
}
