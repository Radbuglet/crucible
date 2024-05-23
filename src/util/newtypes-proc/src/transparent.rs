use proc_macro2::TokenStream;
use quote::{quote_spanned, ToTokens};
use syn::spanned::Spanned;

use crate::Emitter;

use self::custom_syntax::ReprTransparentMeta;

mod custom_syntax {
    syn::custom_keyword!(transparent);
    syn::custom_keyword!(repr);

    pub struct ReprTransparentMeta {
        pub repr: repr,
        pub paren_token: syn::token::Paren,
        pub transparent: transparent,
    }

    impl syn::parse::Parse for ReprTransparentMeta {
        fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
            let in_trans;
            Ok(Self {
                repr: input.parse()?,
                paren_token: syn::parenthesized!(in_trans in input),
                transparent: in_trans.parse()?,
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
    let value_field = syn::parse2::<syn::Ident>(attrs)
        .map_err(|err| {
            emitter.err(err);
        })
        .ok();

    let input = syn::parse2::<syn::DeriveInput>(input.clone())
        .map_err(|err| {
            emitter.err(err);
        })
        .ok();

    let (Some(value_field), Some(input)) = (value_field, input) else {
        return emitter.finish();
    };

    // Scan our inputs for the necessary information
    if !input
        .attrs
        .iter()
        .any(|attr| syn::parse2::<ReprTransparentMeta>(attr.meta.to_token_stream()).is_ok())
    {
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
        if main_field.is_none() && field.ident.as_ref() == Some(&value_field) {
            main_field = Some(&field.ty);
        } else {
            other_fields.push(&field.ty);
        }
    }

    let Some(main_field) = main_field else {
        emitter.err(syn::Error::new_spanned(
            &value_field,
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

        emitter.push(quote_spanned! { value_field.span() =>
            #[allow(dead_code)]
            impl #impl_generics #name #ty_generics #where_clause {
                fn transparent_from_ref(inner: &#main_field) -> &Self {
                    #trans_asserts

                    unsafe { &*(inner as *const #main_field as *const Self) }
                }

                fn transparent_from_mut(inner: &mut #main_field) -> &mut Self {
                    unsafe { &mut *(inner as *mut #main_field as *mut Self) }
                }
            }
        });
    }

    emitter.finish()
}
