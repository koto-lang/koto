//! The `koto` core library module

use crate::prelude::*;
use std::hash::{Hash, Hasher};

/// Initializes the `koto` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.koto");

    result.insert("args", KValue::Tuple(KTuple::default()));

    result.add_fn("copy", |ctx| match ctx.args() {
        [KValue::Iterator(iter)] => Ok(iter.make_copy()?.into()),
        [KValue::List(l)] => Ok(KList::with_data(l.data().clone()).into()),
        [KValue::Map(m)] => {
            let result = KMap::with_contents(
                m.data().clone(),
                m.meta_map().map(|meta| meta.borrow().clone()),
            );
            Ok(result.into())
        }
        [KValue::Object(o)] => o.try_borrow().map(|o| o.copy().into()),
        [other] => Ok(other.clone()),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_fn("deep_copy", |ctx| match ctx.args() {
        [value] => value.deep_copy(),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_fn("exports", |ctx| Ok(KValue::Map(ctx.vm.exports().clone())));

    result.add_fn("hash", |ctx| match ctx.args() {
        [value] => match ValueKey::try_from(value.clone()) {
            Ok(key) => {
                let mut hasher = KotoHasher::default();
                key.hash(&mut hasher);
                Ok(hasher.finish().into())
            }
            Err(_) => Ok(KValue::Null),
        },
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.insert("script_dir", KValue::Null);
    result.insert("script_path", KValue::Null);

    result.add_fn("type", |ctx| match ctx.args() {
        [value] => Ok(value.type_as_string().into()),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result
}
