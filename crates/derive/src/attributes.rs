use syn::{Attribute, LitStr};

#[derive(Default)]
pub(crate) struct KotoAttributes {
    pub type_name: Option<String>,
    pub use_copy: bool,
}

pub(crate) fn koto_derive_attributes(attrs: &[Attribute]) -> KotoAttributes {
    let mut result = KotoAttributes::default();

    for attr in attrs.iter().filter(|a| a.path().is_ident("koto")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("type_name") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                result.type_name = Some(s.value());
                Ok(())
            } else if meta.path.is_ident("use_copy") {
                result.use_copy = true;
                Ok(())
            } else {
                Err(meta.error("unsupported koto attribute"))
            }
        })
        .expect("failed to parse koto attribute");
    }

    result
}
