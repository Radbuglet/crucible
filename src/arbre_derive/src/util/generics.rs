use syn::{GenericParam, Generics, TypeParam};

pub fn remove_defaults(generics: &Generics) -> Generics {
    Generics {
        lt_token: generics.lt_token.clone(),
        params: generics
            .params
            .iter()
            .map(|param| match param {
                GenericParam::Type(param) => GenericParam::Type(TypeParam {
                    attrs: param.attrs.clone(),
                    ident: param.ident.clone(),
                    colon_token: param.colon_token.clone(),
                    bounds: param.bounds.clone(),
                    eq_token: None,
                    default: None,
                }),
                _ => param.clone(),
            })
            .collect(),
        gt_token: generics.gt_token.clone(),
        where_clause: generics.where_clause.clone(),
    }
}
