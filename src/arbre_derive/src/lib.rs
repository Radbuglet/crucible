use proc_macro::{TokenStream as RustTokenStream};

#[proc_macro_derive(ObjDecl, attributes(root, extends, expose))]
pub fn derive_obj_decl(_item: RustTokenStream) -> RustTokenStream {
    todo!()
}

#[proc_macro_attribute]
pub fn methods_proxy(_attr: RustTokenStream, _item: RustTokenStream) -> RustTokenStream {
    todo!()
}
