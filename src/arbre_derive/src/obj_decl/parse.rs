//! ADTs for custom syntax

use syn::parse::{Parse, ParseBuffer};
use syn::{parenthesized, punctuated::Punctuated, token, Expr, Result as SynResult, Token, Type};

pub type ExposeAttrMeta = AttrMeta<Type>;
pub type ExtendsAttrMeta = AttrMeta<Expr>;

pub enum AttrMeta<T> {
    Customized {
        paren: token::Paren,
        list: Punctuated<T, Token![,]>,
    },
    Default,
}

impl<T: Parse> Parse for AttrMeta<T> {
    fn parse(input: &ParseBuffer) -> SynResult<Self> {
        if input.is_empty() {
            Ok(Self::Default)
        } else {
            let inner_tree;
            Ok(Self::Customized {
                paren: parenthesized! { inner_tree in input },
                list: Punctuated::parse_terminated(&inner_tree)?,
            })
        }
    }
}
