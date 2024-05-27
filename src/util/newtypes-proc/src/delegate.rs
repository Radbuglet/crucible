use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};

#[derive(Clone)]
pub struct DelegateInput {
    // #[...]
    pub attrs: Vec<syn::Attribute>,

    // pub
    pub vis: syn::Visibility,

    // fn
    pub fn_kw: syn::Token![fn],

    // fn_name
    pub name: syn::Ident,

    // <'a, T, V: Clause, ...>
    pub generics: syn::Generics,

    // ['a, 'b, ...]
    pub hrtb_bracket: syn::token::Bracket,
    pub hrtb_lts: syn::punctuated::Punctuated<syn::Lifetime, syn::Token![,]>,

    // (a: Ty1, b: Ty2, ...)
    pub params_paren: syn::token::Paren,
    pub params: syn::punctuated::Punctuated<DelegateParam, syn::Token![,]>,

    // -> Out
    pub result: syn::ReturnType,

    // ;
    pub semi: syn::Token![;],
}

impl Parse for DelegateInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;

        let vis = input.parse()?;

        let fn_kw = input.parse()?;

        let name = input.parse()?;

        let generics = input.parse()?;

        let hrtb_group;
        let hrtb_bracket = syn::bracketed!(hrtb_group in input);
        let hrtb_lts = syn::punctuated::Punctuated::parse_terminated(&hrtb_group)?;

        let params_group;
        let params_paren = syn::parenthesized!(params_group in input);
        let params = syn::punctuated::Punctuated::parse_terminated(&params_group)?;

        let result = input.parse()?;

        let generics = syn::Generics {
            where_clause: input.parse()?,
            ..generics
        };

        let semi = input.parse()?;

        Ok(Self {
            attrs,
            vis,
            fn_kw,
            name,
            generics,
            hrtb_bracket,
            hrtb_lts,
            params_paren,
            params,
            result,
            semi,
        })
    }
}

#[derive(Clone)]
pub struct DelegateParam {
    pub name: syn::Ident,
    pub colon: syn::token::Colon,
    pub ty: syn::Type,
}

impl Parse for DelegateParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            name: input.parse()?,
            colon: input.parse()?,
            ty: input.parse()?,
        })
    }
}

pub fn delegate(input: TokenStream) -> TokenStream {
    let input = match syn::parse2::<DelegateInput>(input) {
        Ok(input) => input,
        Err(err) => {
            return err.into_compile_error();
        }
    };

    let attrs = &input.attrs;
    let vis = &input.vis;
    let name = &input.name;
    let fmt_str = Literal::string(&format!("{name}({{:p}}) @ {{}}"));
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let param_trait = {
        let hrtbs = input.hrtb_lts.iter();
        let args = input.params.iter().map(|v| &v.ty);
        let result = &input.result;

        quote! {
            for<#(#hrtbs),*> ::core::ops::Fn(#(#args),*) #result + ::core::marker::Send + ::core::marker::Sync + 'static
        }
    };

    quote! {
        #(#attrs)*
        #vis struct #name #impl_generics #where_clause {
            defined_at: &'static ::core::panic::Location<'static>,
            func: ::std::sync::Arc<dyn #param_trait>,
        }

        impl #impl_generics ::core::fmt::Debug for #name #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::write!(f, #fmt_str, &self.func, &self.defined_at)
            }
        }

        impl #impl_generics ::core::clone::Clone for #name #ty_generics #where_clause {
            fn clone(&self) -> Self {
                Self {
                    defined_at: self.defined_at,
                    func: ::core::clone::Clone::clone(&self.func),
                }
            }
        }

        impl #impl_generics ::core::ops::Deref for #name #ty_generics #where_clause {
            type Target = dyn #param_trait;

            fn deref(&self) -> &Self::Target {
                &*self.func
            }
        }

        impl #impl_generics #name #ty_generics #where_clause {
            #[track_caller]
            pub fn new(f: impl #param_trait) -> Self {
                Self {
                    defined_at: ::core::panic::Location::caller(),
                    func: ::std::sync::Arc::new(f),
                }
            }
        }
    }
}
