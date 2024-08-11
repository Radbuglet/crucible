use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};

#[derive(Clone)]
pub struct MultiClosureInput {
    // #[...]
    pub attrs: Vec<syn::Attribute>,

    // pub
    pub vis: syn::Visibility,

    // enum
    pub enum_kw: syn::Token![enum],

    // item_name
    pub name: syn::Ident,

    // <'a, T, V: Clause, ...> ... where
    pub generics: syn::Generics,

    // { foo(a, b, c) -> d, bar(a, b) -> (c, d) }
    pub body_brace: syn::token::Brace,
    pub body: syn::punctuated::Punctuated<MultiClosureFunc, syn::Token![,]>,
}

impl Parse for MultiClosureInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;

        let vis = input.parse()?;

        let enum_kw = input.parse()?;

        let name = input.parse()?;

        let generics = input.parse()?;
        let generics = syn::Generics {
            where_clause: input.parse()?,
            ..generics
        };

        let body_group;
        let body_brace = syn::braced!(body_group in input);
        let body = syn::punctuated::Punctuated::parse_terminated(&body_group)?;

        Ok(Self {
            attrs,
            vis,
            enum_kw,
            name,
            generics,
            body_brace,
            body,
        })
    }
}

#[derive(Clone)]
pub struct MultiClosureFunc {
    // name
    pub name: syn::Ident,

    // (A, B, C, D<u32>)
    pub params_paren: syn::token::Paren,
    pub params: syn::punctuated::Punctuated<syn::Type, syn::Token![,]>,

    // -> Out
    pub result: syn::ReturnType,
}

impl Parse for MultiClosureFunc {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let params_group;

        Ok(Self {
            name: input.parse()?,
            params_paren: syn::parenthesized!(params_group in input),
            params: syn::punctuated::Punctuated::parse_terminated(&params_group)?,
            result: input.parse()?,
        })
    }
}

pub fn multi_closure(input: TokenStream) -> TokenStream {
    // Parse input
    let mut input = match syn::parse2::<MultiClosureInput>(input) {
        Ok(input) => input,
        Err(err) => {
            return err.into_compile_error();
        }
    };

    // Introduce a new lifetime parameter to denote the lifetime out of our-pointer.
    let out_lt = syn::Lifetime::new("'__out", input.name.span());
    input.generics.params.insert(
        0,
        syn::GenericParam::Lifetime(syn::LifetimeParam::new(out_lt.clone())),
    );

    let vis = &input.vis;
    let item_name = &input.name;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_generics) = generics.split_for_impl();

    let mut ty_generics_infer_out = generics.clone();
    ty_generics_infer_out.params[0] = syn::GenericParam::Lifetime(syn::LifetimeParam::new(
        syn::Lifetime::new("'_", Span::call_site()),
    ));
    let ty_generics_infer_out = ty_generics_infer_out.split_for_impl().1;

    let mut enum_variants = TokenStream::new();
    let mut methods = TokenStream::new();

    for func in &input.body {
        let name = &func.name;
        let name_ctor = syn::Ident::new(&format!("new_{}", name), name.span());
        let name_call = syn::Ident::new(&format!("call_{}", name), name.span());
        let inputs = &func.params;
        let result = &func.result;

        let result_ty = match result {
            syn::ReturnType::Default => quote! {()},
            syn::ReturnType::Type(_, ty) => ty.to_token_stream(),
        };

        let inputs_tup = syn::TypeTuple {
            paren_token: syn::token::Paren(Span::call_site()),
            elems: inputs.clone(),
        };

        quote! {
            #name {
                inputs: #inputs_tup,
                output_marker: [(fn() #result, &#out_lt mut ()); 0],
                output_ptr: *mut (),
            },
        }
        .to_tokens(&mut enum_variants);

        let mut input_forwards = TokenStream::new();
        for i in 0..inputs.len() {
            let i = syn::Index::from(i);
            quote! { inputs.#i, }.to_tokens(&mut input_forwards);
        }

        quote! {
            pub fn #name_ctor(inputs: #inputs_tup, output: &#out_lt mut ::core::option::Option<#result_ty>) -> Self {
                Self::#name { inputs, output_marker: [], output_ptr: output as *mut _ as *mut () }
            }

            pub fn #name_call(inputs: #inputs_tup, f: impl ::core::ops::FnOnce(#item_name #ty_generics_infer_out)) -> #result_ty {
                let mut out = ::core::option::Option::None;
                f(#item_name::#name_ctor(inputs, &mut out));
                out.expect("result was never provided")
            }

            pub fn #name(self, f: impl ::core::ops::FnOnce(#inputs) #result) -> Self {
                match self {
                    Self::#name { inputs, output_ptr, .. } => {
                        unsafe {
                            *(output_ptr as *mut ::core::option::Option<#result_ty>) =
                                ::core::option::Option::Some(f(#input_forwards));
                        }
                        Self::__AlreadyDispatched
                    },
                    other @ _ => other,
                }
            }
        }
        .to_tokens(&mut methods);
    }

    quote! {
        #[allow(non_camel_case_types)]
        #vis enum #item_name #generics {
            __AlreadyDispatched,
            #enum_variants
        }

        impl #impl_generics #item_name #ty_generics #where_generics {
            #methods
        }
    }
}
