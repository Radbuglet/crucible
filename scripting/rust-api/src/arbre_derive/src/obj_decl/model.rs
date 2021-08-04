//! An intermediate representation of the derive invocation with methods to generate `TokenTree`
//! fragments.

use crate::util::field::AnyField;
use crate::util::MyTokenStreamExt;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Expr, Type};

pub struct ModulePaths {
    fetch: TokenStream,
    vtable: TokenStream,
}

impl ModulePaths {
    pub fn new(base: TokenStream) -> Self {
        let fetch = quote! { #base::fetch };
        let vtable = quote! { #base::vtable };
        Self { fetch, vtable }
    }

    pub fn fetch(&self) -> &TokenStream {
        &self.fetch
    }

    pub fn vtable(&self) -> &TokenStream {
        &self.vtable
    }
}

#[derive(Clone)]
pub struct TableEntry {
    pub target: EntryTarget,
    pub action: EntryAction,
}

impl TableEntry {
    pub fn as_builder_command(&self, paths: &ModulePaths) -> TokenStream {
        let getter = self.target.getter(paths);
        let span = self.target.span();

        match &self.action {
            EntryAction::ExposeDefault => quote_spanned! { span => .expose(#getter) },
            EntryAction::ExposeUnsized(target_ty) => {
                quote_spanned! { span => .expose_unsized::<_, #target_ty>(#getter) }
            }
            EntryAction::ExtendsDefault => quote_spanned! { span => .extends_default(#getter) },
            EntryAction::ExtendsCustom(table_expr) => {
                quote_spanned! { span => .extends(#getter, #table_expr) }
            }
        }
    }
}

#[derive(Clone)]
pub enum EntryTarget {
    Identity(Span),
    Field(AnyField),
}

impl EntryTarget {
    pub fn getter(&self, paths: &ModulePaths) -> TokenStream {
        match self {
            EntryTarget::Identity(span) => {
                let p_mod_vtable = paths.vtable().clone().with_span(*span);
                quote_spanned! { *span => #p_mod_vtable::identity_field::<Self>() }
            }
            EntryTarget::Field(field) => {
                let p_mod_vtable = paths.vtable().clone().with_span(field.span);
                let name = &field.name;
                quote_spanned! { field.span => #p_mod_vtable::get_field!(Self, #name) }
            }
        }
    }

    pub fn span(&self) -> Span {
        match self {
            EntryTarget::Identity(span) => *span,
            EntryTarget::Field(field) => field.span,
        }
    }
}

#[derive(Clone)]
pub enum EntryAction {
    ExposeDefault, // TODO: Keyed version
    ExposeUnsized(Type),
    ExtendsDefault,
    ExtendsCustom(Expr),
}
