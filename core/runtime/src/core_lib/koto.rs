//! The `koto` core library module

use crate::prelude::*;
use std::hash::{Hash, Hasher};

/// Initializes the `koto` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.koto");

    result.add_value("args", Value::Tuple(KTuple::default()));

    result.add_fn("copy", |ctx| match ctx.args() {
        [Value::Iterator(iter)] => Ok(iter.make_copy()?.into()),
        [Value::List(l)] => Ok(KList::with_data(l.data().clone()).into()),
        [Value::Map(m)] => {
            let result = KMap::with_contents(
                m.data().clone(),
                m.meta_map().map(|meta| meta.borrow().clone()),
            );
            Ok(result.into())
        }
        [Value::Object(o)] => o.try_borrow().map(|o| o.copy().into()),
        [other] => Ok(other.clone()),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_fn("deep_copy", |ctx| match ctx.args() {
        [value] => value.deep_copy(),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_fn("exports", |ctx| Ok(Value::Map(ctx.vm.exports().clone())));

    result.add_fn("hash", |ctx| match ctx.args() {
        [value] => match ValueKey::try_from(value.clone()) {
            Ok(key) => {
                let mut hasher = KotoHasher::default();
                key.hash(&mut hasher);
                Ok(hasher.finish().into())
            }
            Err(_) => Ok(Value::Null),
        },
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_value("script_dir", Value::Null);
    result.add_value("script_path", Value::Null);

    result.add_fn("type", |ctx| match ctx.args() {
        [value] => Ok(value.type_as_string().into()),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result
}
