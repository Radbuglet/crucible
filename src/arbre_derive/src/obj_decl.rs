use proc_macro2::TokenStream;
use syn::{Result as SynResult, Error as SynError, parse2, DeriveInput, GenericParam, TypeParam};
use crate::util::attr::{HelperAttr, parse_attrs_subset, parse_attrs_single};
use crate::util::meta_enum::meta_enum;
use syn::spanned::Spanned;

meta_enum! {
    enum(HelperAttr) DeriveAttrs {
        Root = HelperAttr {
            name: "root",
            loc_hint: "`root` can only show up on the struct's generic type parameters."
        },
        Expose = HelperAttr {
            name: "expose",
            loc_hint: "`expose` can only show up on the struct's fields."
        },
        Extends = HelperAttr {
            name: "extends",
            loc_hint: "`extends` can only show up on the struct's fields."
        }
    }
}

pub fn derive(item: TokenStream) -> SynResult<TokenStream> {
    let item = parse2::<DeriveInput>(item)?;

    let root_attr = {
        let mut found = Option::<&TypeParam>::None;

        for param in &item.generics.params {
            // Parse parameter type
            let param = match param {
                GenericParam::Type(param) => {
                    Some(param)
                }
                GenericParam::Lifetime(param) => {
                    // Lifetime parameters have no applicable attributes
                    let _ = parse_attrs_subset::<DeriveAttrs>(&param.attrs, &[])?;
                    None
                }
                GenericParam::Const(param) => {
                    // Const parameters have no applicable attributes
                    let _ = parse_attrs_subset::<DeriveAttrs>(&param.attrs, &[])?;
                    None
                }
            };

            // Check if the parameter is root
            if let Some(param) = param {
                let root = parse_attrs_single(&param.attrs, DeriveAttrs::Root)?;
                if root.is_some() {
                    // Check that we don't already have a root parameter
                    if let Some(_) = found {
                        return Err(SynError::new(param.span(), "At most one type parameter can be marked as `root`."));
                    }
                    found = Some(param);
                }
            }
        }
        found
    };

    if root_attr.is_some() {
        println!("We have a root!");
    } else {
        println!("We don't have a root!");
    }

    Ok(TokenStream::new())
}
