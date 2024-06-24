use proc_macro2::TokenStream;
use quote::{quote_spanned, ToTokens};
use syn::spanned::Spanned;

use crate::util::Emitter;

mod custom_syntax {
    use syn::parse;

    syn::custom_keyword!(transparent);

    #[derive(Clone)]
    pub struct MacroArg {
        pub on_field: syn::Ident,
        pub comma: syn::token::Comma,
        pub prefix_vis: syn::Visibility,
        pub prefix_ident: syn::Ident,
    }

    impl parse::Parse for MacroArg {
        fn parse(input: parse::ParseStream) -> syn::Result<Self> {
            Ok(Self {
                on_field: input.parse()?,
                comma: input.parse()?,
                prefix_vis: input.parse()?,
                prefix_ident: input.parse()?,
            })
        }
    }
}

pub fn transparent(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::default();

    // Always ensure that we emit out the original structure so rust can generate diagnostics for it
    // and so rust-analyzer can auto-complete on it.
    emitter.push(&input);

    // Parse our inputs
    let attrs = syn::parse2::<custom_syntax::MacroArg>(attrs)
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

    // Scan our inputs for the necessary information
    if !input.attrs.iter().any(|attr| {
        attr.path().is_ident(&"repr") && attr.parse_args::<custom_syntax::transparent>().is_ok()
    }) {
        emitter.err(syn::Error::new_spanned(
            &input.ident,
            "`transparent` attribute is only applicable to structs with the `#[repr(transparent)]` attribute",
        ));
    }

    let syn::Data::Struct(input_data) = &input.data else {
        emitter.err(syn::Error::new(
            match &input.data {
                syn::Data::Struct(_) => unreachable!(),
                syn::Data::Enum(variant) => variant.enum_token.span,
                syn::Data::Union(variant) => variant.union_token.span,
            },
            "`transparent` attribute is only applicable to `struct`s",
        ));
        return emitter.finish();
    };

    let mut main_field = None;
    let mut other_fields = Vec::new();

    for field in &input_data.fields {
        if main_field.is_none() && field.ident.as_ref() == Some(&attrs.on_field) {
            main_field = Some(&field.ty);
        } else {
            other_fields.push(&field.ty);
        }
    }

    let Some(main_field) = main_field else {
        emitter.err(syn::Error::new_spanned(
            &attrs.on_field,
            "failed to find a field with this name in the struct",
        ));
        return emitter.finish();
    };

    // Generate transparency assertions
    let mut trans_asserts = TokenStream::default();

    for (i, field) in other_fields.iter().enumerate() {
        let orig_name = &input.ident;
        let name = syn::Ident::new(&format!("Validator{i}"), field.span());
        let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

        quote_spanned! { field.span() =>
            #[repr(transparent)]
            struct #name #impl_generics #where_clause {
                _allegedly_zst: #field,
                _non_zst_and_binder: #orig_name #ty_generics,
            }
        }
        .to_tokens(&mut trans_asserts);
    }

    // Generate conversion methods
    {
        let name = &input.ident;
        let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

        let prefix = &attrs.prefix_ident;

        let wrap_vis = &attrs.prefix_vis;
        let wrap_ref = syn::Ident::new(&format!("{prefix}_ref"), attrs.prefix_ident.span());
        let wrap_mut = syn::Ident::new(&format!("{prefix}_mut"), attrs.prefix_ident.span());

        emitter.push(quote_spanned! { attrs.on_field.span() =>
            #[allow(dead_code, clippy::needless_lifetimes)]
            impl #impl_generics #name #ty_generics #where_clause {
                #wrap_vis const fn #wrap_ref<'__transparent_lt>(inner: &'__transparent_lt #main_field) -> &'__transparent_lt Self {
                    #trans_asserts

                    unsafe { &*(inner as *const #main_field as *const Self) }
                }

                #wrap_vis fn #wrap_mut<'__transparent_lt>(inner: &'__transparent_lt mut #main_field) -> &'__transparent_lt mut Self {
                    unsafe { &mut *(inner as *mut #main_field as *mut Self) }
                }
            }
        });
    }

    emitter.finish()
}