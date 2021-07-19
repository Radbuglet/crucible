use crate::util::attr::{parse_attrs_single, parse_attrs_subset, HelperAttr};
use crate::util::generics::remove_defaults;
use crate::util::meta_enum::meta_enum;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    parse2, Attribute, Data, DeriveInput, Error as SynError, Fields, GenericParam,
    Result as SynResult, TypeParam,
};

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

struct TableEntry {
    getter: TokenStream,
    action: TableAction,
}

// TODO: Add support for non-default entries
enum TableAction {
    ExposeDefault,
    ExtendsDefault,
}

pub fn derive(item: TokenStream) -> SynResult<TokenStream> {
    let item = parse2::<DeriveInput>(item)?;

    // Path declarations
    let p_crate = quote! { ::arbre };
    let p_fetch = quote! { #p_crate::fetch };
    let p_table = quote! { #p_crate::vtable };

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
        &quote_spanned! { item.ident.span() => #p_table::identity_field::<Self>() },
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
        match &data.fields {
            Fields::Named(fields) => {
                for field in &fields.named {
                    let _ = collect_field_actions(
                        &mut entries,
                        &field.attrs,
                        &quote_spanned! { field.span() => #p_table::get_field!(Self, #(field.ident)) },
                    )?;
                }
            }
            Fields::Unnamed(fields) => {
                for (index, field) in fields.unnamed.iter().enumerate() {
                    let _ = collect_field_actions(
                        &mut entries,
                        &field.attrs,
                        &quote_spanned! { field.span() => #p_table::get_field!(Self, #index) },
                    )?;
                }
            }
            Fields::Unit => {}
        };
    }

    // Generate code
    let generics = remove_defaults(&item.generics);
    let root_ty = match root_attr {
        Some(attr) => {
            let ident = &attr.ident;
            quote! { #ident }
        }
        None => quote! { dyn #p_fetch::Obj },
    };
    let table_statements = entries.iter().map(|entry| {
        let getter = &entry.getter;
        match &entry.action {
            TableAction::ExposeDefault => quote! { .expose(#getter) },
            TableAction::ExtendsDefault => quote! { .extend_default(#getter) },
        }
    });

    Ok(quote! {
        impl<#generics.params> #p_fetch::ObjDecl for #item.ident #generics.where_clause {
            type Root = #root_ty;
            const TABLE: #p_table::VTable<Self, Self::Root> = #p_table::VTableBuilder::new()
                #(#table_statements)*
                .into_inner();
        }
    })
}

fn collect_field_actions(
    entries: &mut Vec<TableEntry>,
    attrs: &Vec<Attribute>,
    getter: &TokenStream,
) -> SynResult<()> {
    for (key, _attr) in parse_attrs_subset(attrs, &[DeriveAttrs::Expose, DeriveAttrs::Extends])? {
        let action = match key {
            DeriveAttrs::Expose => TableAction::ExposeDefault,
            DeriveAttrs::Extends => TableAction::ExtendsDefault,
            _ => unreachable!(),
        };
        entries.push(TableEntry {
            getter: getter.clone(),
            action,
        });
    }

    Ok(())
}
