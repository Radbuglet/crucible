use crate::util::attr::{parse_attrs_single, parse_attrs_subset, HelperAttr};
use crate::util::field::collect_fields;
use crate::util::meta_enum::meta_enum;
use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{
    parse2, Attribute, Data, DeriveInput, Error as SynError, GenericParam, Result as SynResult,
    TypeParam,
};

mod parse;
use parse::*;
mod model;
use model::*;

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
    let paths = ModulePaths::new(quote! { ::arbre });

    // Find root type parameter
    let root_attr = {
        let mut found = Option::<&TypeParam>::None;

        for param in &item.generics.params {
            // Parse parameter type
            let param = match param {
                GenericParam::Type(param) => Some(param),
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
                        return Err(SynError::new(
                            param.span(),
                            "At most one type parameter can be marked as `root`.",
                        ));
                    }
                    found = Some(param);
                }
            }
        }
        found
    };

    // Collect entries
    let mut entries = Vec::new();
    let _ = collect_field_actions(
        &mut entries,
        &item.attrs,
        &EntryTarget::Identity(item.ident.span()),
    )?;

    {
        // Parse struct type
        let data = match &item.data {
            Data::Struct(data) => data,
            Data::Enum(data) => {
                return Err(SynError::new(
                    data.enum_token.span(),
                    "`ObjDecl` cannot be derived for enums.",
                ))
            }
            Data::Union(data) => {
                return Err(SynError::new(
                    data.union_token.span(),
                    "`ObjDecl` cannot be derived for unions.",
                ))
            }
        };

        // Collect entries from field attributes
        // TODO: Check for inappropriate attribute use in field types
        for field in collect_fields(&data.fields) {
            let _ = collect_field_actions(
                &mut entries,
                &field.attrs,
                &EntryTarget::Field(field.clone()),
            );
        }
    }

    // === Generate code
    // Get paths
    let p_fetch = paths.fetch();
    let p_table = paths.vtable();

    // Collect impl arguments
    let name = &item.ident;
    let (impl_params, type_params, where_clause) = item.generics.split_for_impl();

    // Build entries and root
    let table_statements = entries.iter().map(|entry| entry.as_builder_command(&paths));
    let root_ty = match root_attr {
        Some(attr) => quote! { #attr.ident },
        None => quote! { dyn #p_fetch::Obj },
    };

    // Build final token tree
    Ok(quote! {
        #[automatically_derived]
        impl #impl_params #p_fetch::ObjDecl for #name #type_params #where_clause {
            type Root = #root_ty;
            const TABLE: #p_table::VTable<Self, Self::Root> = #p_table::VTableBuilder::new()
                #(#table_statements)*
                .into_inner();
        }

        #[automatically_derived]
        impl #impl_params #p_fetch::Comp for #name #type_params #where_clause {
            type Root = #root_ty;
        }
    })
}

fn collect_field_actions(
    entries: &mut Vec<TableEntry>,
    attrs: &Vec<Attribute>,
    target: &EntryTarget,
) -> SynResult<()> {
    for (key, attr) in parse_attrs_subset(attrs, &[DeriveAttrs::Expose, DeriveAttrs::Extends])? {
        match key {
            DeriveAttrs::Expose => match parse2::<ExposeAttrMeta>(attr.tokens.clone())? {
                AttrMeta::Customized {
                    list: expose_as, ..
                } => {
                    for alias in expose_as {
                        entries.push(TableEntry {
                            target: target.clone(),
                            action: EntryAction::ExposeUnsized(alias),
                        });
                    }
                }
                AttrMeta::Default => {
                    entries.push(TableEntry {
                        target: target.clone(),
                        action: EntryAction::ExposeDefault,
                    });
                }
            },
            DeriveAttrs::Extends => match parse2::<ExtendsAttrMeta>(attr.tokens.clone())? {
                AttrMeta::Customized {
                    list: extends_using,
                    ..
                } => {
                    for expr in extends_using {
                        entries.push(TableEntry {
                            target: target.clone(),
                            action: EntryAction::ExtendsCustom(expr),
                        });
                    }
                }
                AttrMeta::Default => {
                    entries.push(TableEntry {
                        target: target.clone(),
                        action: EntryAction::ExtendsDefault,
                    });
                }
            },
            _ => unreachable!(),
        };
    }

    Ok(())
}
