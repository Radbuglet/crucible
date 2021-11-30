use proc_macro2::{Ident, Span};
use syn::spanned::Spanned;
use syn::{Attribute, Fields, Type};

#[derive(Clone)]
pub struct AnyField {
    pub name: Ident,
    pub span: Span,
    pub attrs: Vec<Attribute>,
    pub type_: Type,
}

pub fn collect_fields(fields: &Fields) -> Vec<AnyField> {
    match fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .map(|field| AnyField {
                name: field.ident.clone().unwrap(),
                span: field.span(),
                attrs: field.attrs.clone(),
                type_: field.ty.clone(),
            })
            .collect(),
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(index, field)| AnyField {
                name: Ident::new(index.to_string().as_str(), field.span()),
                span: field.span(),
                attrs: field.attrs.clone(),
                type_: field.ty.clone(),
            })
            .collect(),
        Fields::Unit => Vec::new(),
    }
}
