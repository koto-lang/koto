//! The `koto` core library module

use {
    crate::prelude::*,
    std::hash::{Hash, Hasher},
};

/// Initializes the `koto` core library module
pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::new();

    result.add_value("args", Tuple(ValueTuple::default()));

    result.add_fn("copy", |vm, args| match vm.get_args(args) {
        [Iterator(iter)] => Ok(iter.make_copy()?.into()),
        [List(l)] => Ok(ValueList::with_data(l.data().clone()).into()),
        [Map(m)] => {
            let result = ValueMap::with_contents(
                m.data().clone(),
                m.meta_map().map(|meta| meta.borrow().clone()),
            );
            Ok(result.into())
        }
        [Object(o)] => o.try_borrow().map(|o| o.copy().into()),
        [other] => Ok(other.clone()),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_fn("deep_copy", |vm, args| match vm.get_args(args) {
        [value] => value.deep_copy(),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_fn("exports", |vm, _| Ok(Map(vm.exports().clone())));

    result.add_fn("hash", |vm, args| match vm.get_args(args) {
        [value] => match ValueKey::try_from(value.clone()) {
            Ok(key) => {
                let mut hasher = KotoHasher::default();
                key.hash(&mut hasher);
                Ok(hasher.finish().into())
            }
            Err(_) => Ok(Null),
        },
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_value("script_dir", Null);
    result.add_value("script_path", Null);

    result.add_fn("type", |vm, args| match vm.get_args(args) {
        [value] => Ok(value.type_as_string().into()),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result
}
