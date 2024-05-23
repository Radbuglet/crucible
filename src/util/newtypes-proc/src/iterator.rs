use proc_macro2::TokenStream;
use quote::quote;

use crate::Emitter;

#[derive(Clone)]
struct AttrArgs {
    item_ty: syn::Type,
    _comma: syn::token::Comma,
    getter: syn::Expr,
}

impl syn::parse::Parse for AttrArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            item_ty: input.parse()?,
            _comma: input.parse()?,
            getter: input.parse()?,
        })
    }
}

pub fn iterator(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::default();

    // Always ensure that we emit out the original structure so rust can generate diagnostics for it
    // and so rust-analyzer can auto-complete on it.
    emitter.push(&input);

    // Parse our inputs
    let attrs = syn::parse2::<AttrArgs>(attrs)
        .map_err(|err| {
            emitter.err(err);
        })
        .ok();

    let input = syn::parse2::<syn::DeriveInput>(input.clone())
        .map_err(|err| {
            emitter.err(err);
        })
        .ok();

    let (Some(attrs), Some(input)) = (attrs, input) else {
        return emitter.finish();
    };

    // Emit iterator implementation
    {
        let name = &input.ident;
        let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
        let ty = &attrs.item_ty;
        let getter = &attrs.getter;

        emitter.push(quote! {
            impl #impl_generics ::core::iter::Iterator for #name #ty_generics #where_clause {
                type Item = #ty;

                fn next(&mut self) -> ::core::option::Option<Self::Item> {
                    ::core::iter::Iterator::next(#getter)
                }
            }
        });
    }

    emitter.finish()
}
