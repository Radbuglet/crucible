#![feature(decl_macro)]

use proc_macro::TokenStream as RustTokenStream;
use syn::Error as SynError;

mod obj_decl;
mod util;

#[proc_macro_derive(ObjDecl, attributes(root, expose, extends))]
pub fn derive_obj_decl(item: RustTokenStream) -> RustTokenStream {
    obj_decl::derive(item.into())
        .unwrap_or_else(SynError::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn methods_proxy(_attr: RustTokenStream, _item: RustTokenStream) -> RustTokenStream {
    todo!()
}
