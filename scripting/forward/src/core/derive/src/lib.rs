#![feature(proc_macro_diagnostic)]

use proc_macro::TokenStream as TokenStreamNative;
use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::{parse2, Pat, Result as SynResult};
use syn::{FnArg, GenericParam, ItemTrait, TraitItem};

#[proc_macro_attribute]
pub fn forward(
    attr_stream: TokenStreamNative,
    block_stream: TokenStreamNative,
) -> TokenStreamNative {
    match forward_inner(attr_stream.into(), block_stream.into()) {
        Ok(stream) => {
            // println!("{}", stream);
            stream
        }
        Err(err) => err.to_compile_error(),
    }
    .into()
}

// TODO: Verify hygiene, make more robust, and make spans more useful.
// TODO: Derive for `T: Deref`.
// TODO: Expand support for pattern arguments.
// TODO: Add support for `impl` block derivations.
// TODO: Allow trait renames.
fn forward_inner(attr_stream: TokenStream, block_stream: TokenStream) -> SynResult<TokenStream> {
    let main_trait = parse2::<ItemTrait>(block_stream.clone())?;

    //> Parse attributes
    if !attr_stream.is_empty() {
        attr_stream.span().unwrap().warning("Unexpected.").emit();
    }

    //> Isolate generic declaration
    let main_name = &main_trait.ident;
    let vis = &main_trait.vis;
    let fwd_name = Ident::new(
        format!("{}F", main_trait.ident).as_str(),
        main_trait.ident.span(),
    );

    //> Collect and forward generic parameters

    // The where clause of the original trait
    let where_sig = match main_trait.generics.where_clause {
        Some(clause) => clause.to_token_stream(),
        None => quote! {},
    };

    // Generic parameter forwarding arguments
    let params_pass = {
        let params = main_trait.generics.params.iter().map(|param| match param {
            GenericParam::Lifetime(lt) => lt.lifetime.to_token_stream(),
            GenericParam::Type(ty) => ty.ident.to_token_stream(),
            GenericParam::Const(cst) => {
                let name = &cst.ident;
                quote! { {#name} }
            }
        });

        quote! { #(#params),* }
    };

    // A list of generic parameters and their bounds.
    let main_params_sig = &main_trait.generics.params;

    // A list of generic parameters and their bounds with the "__Target" impl parameter.
    let fwd_params_sig = {
        let mut iter = main_params_sig.into_iter();
        let mut needs_target = true;
        let mut params = Vec::new();

        loop {
            let next = iter.next();

            // Pre-process the next parameter
            if needs_target {
                match next {
                    // The "__Target" type-parameter must be between lifetimes and const parameters.
                    Some(GenericParam::Const(_)) | None => {
                        params.push(quote! { __Target: #fwd_name<#params_pass> });
                        needs_target = false;
                    }
                    _ => {}
                }
            }

            // Commit this parameter
            match next {
                Some(param) => params.push(param.to_token_stream()),
                None => break,
            }
        }

        quote! { <#(#params),*> }
    };

    //> Construct forwarding items
    let fwd_items = {
        let mut mapped = Vec::new();
        for item in &main_trait.items {
            match item {
                TraitItem::Type(item) => {
                    let name = &item.ident;

                    mapped.push(quote! {
                        type #name = <__Target::Target as #main_name<#params_pass>>::#name;
                    });
                }
                TraitItem::Const(item) => {
                    let name = &item.ident;
                    let ty = &item.ty;

                    mapped.push(quote! {
                        const #name: #ty = <__Target::Target as #main_name<#params_pass>>::#name;
                    });
                }
                TraitItem::Method(item) => {
                    let sig = &item.sig;
                    let fn_name = &sig.ident;
                    let mut param_names = Vec::new();

                    for param in &sig.inputs {
                        match param {
                            FnArg::Typed(param) => {
                                if let Pat::Ident(pat) = &*param.pat {
                                    param_names.push(pat.ident.to_token_stream());
                                } else {
                                    param
                                        .span()
                                        .unwrap()
                                        .error("Unknown parameter pattern")
                                        .emit();
                                }
                            }
                            FnArg::Receiver(recv) => {
                                if let Some((punct, _)) = &recv.reference {
                                    punct.span()
                                        .unwrap()
                                        .error("Forwarded trait methods must take ownership of `self`.")
                                        .emit();
                                }
                            }
                        }
                    }

                    mapped.push(quote! {
                        #[allow(unused_parameters)]
                        #sig {
                            #fwd_name::target(self).#fn_name(#(#param_names)*)
                        }
                    });
                }
                TraitItem::Macro(item) => {
                    item.span().unwrap().error("Unexpected macro").emit();
                }
                TraitItem::Verbatim(item) => {
                    item.span().unwrap().error("Unexpected tokens").emit();
                }
                _ => {}
            }
        }
        mapped
    };

    //> Construct new trait and forwarder.
    Ok(quote! {
        #block_stream

        #vis trait #fwd_name<#main_params_sig> #where_sig {
            type Target: #main_name<#params_pass>;

            fn target(self) -> Self::Target;
        }

        impl #fwd_params_sig #main_name<#params_pass> for __Target #where_sig {
            #(#fwd_items)*
        }
    })
}
