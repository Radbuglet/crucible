use super::meta_enum::MetaEnum;

use syn::{Result as SynResult, Error as SynError, Attribute, Path, AttrStyle};
use syn::spanned::Spanned;

pub struct HelperAttr {
    pub name: &'static str,
    pub loc_hint: &'static str,
}

pub fn decode_attr_path<A>(path: &Path) -> Option<A>
where
    A: MetaEnum<Meta = HelperAttr>
{
    for (key, meta) in A::variants() {
        if path.is_ident(meta.name) {
            return Some(*key);
        }
    }
    None
}

pub fn parse_attrs<A>(attrs: &Vec<Attribute>) -> impl Iterator<Item = (A, &Attribute)>
where
    A: MetaEnum<Meta = HelperAttr>
{
    attrs.iter().filter_map(|attr| {
        if let Some(attr_key) = decode_attr_path::<A>(&attr.path) {
            Some ((attr_key, attr))
        } else {
            None
        }
    })
}

pub fn parse_attrs_subset<'a, A>(attrs: &'a Vec<Attribute>, supported: &[A]) -> SynResult<impl Iterator<Item = (A, &'a Attribute)>>
where
    A: MetaEnum<Meta = HelperAttr>
{
    for (key, attr) in parse_attrs::<A>(attrs) {
        // Check against supported subset
        if !supported.contains(&key) {
            return Err(SynError::new(attr.span(), key.meta().loc_hint));
        }

        // Validate style
        match attr.style {
            AttrStyle::Outer => {}
            _ => return Err(SynError::new(
                attr.span(),
                format!("`{}` must be an outer attribute in this context.", key.meta().name)
            ))
        }
    }

    Ok(parse_attrs(attrs))
}

pub fn parse_attrs_single<A>(attrs: &Vec<Attribute>, supported: A) -> SynResult<Option<(A, &Attribute)>>
where
    A: MetaEnum<Meta = HelperAttr>
{
    let mut attrs = parse_attrs_subset(attrs, &[supported])?;
    let first = attrs.next();

    if let Some((key, duplicate)) = attrs.next() {
        return Err(SynError::new(duplicate.span(), format!("`{}` can only show up once per item.", key.meta().name)));
    }
    Ok(first)
}
