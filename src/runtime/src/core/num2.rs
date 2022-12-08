//! The `num2` core library module

use {
    super::iterator::collect_pair,
    crate::{num2, prelude::*, ValueIteratorOutput as Output},
};

/// Initializes the `num2` core library module
pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::new();

    result.add_fn("angle", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number(n[1].atan2(n[0]).into())),
        unexpected => num2_error(unexpected),
    });

    result.add_fn("length", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number(n.length().into())),
        unexpected => num2_error(unexpected),
    });

    result.add_fn("lerp", |vm, args| match vm.get_args(args) {
        [Num2(a), Num2(b), Number(t)] => {
            let result = *t * (b - a) + a;
            Ok(Num2(result))
        }
        unexpected => type_error_with_slice("(Num2, Num2, Number) as arguments", unexpected),
    });

    result.add_fn("make_num2", |vm, args| {
        let result = match vm.get_args(args) {
            [Number(n)] => num2::Num2(n.into(), n.into()),
            [Number(n1), Number(n2)] => num2::Num2(n1.into(), n2.into()),
            [Num2(n)] => *n,
            [iterable] if iterable.is_iterable() => {
                let iterable = iterable.clone();
                num2_from_iterator(vm.make_iterator(iterable)?)?
            }
            unexpected => {
                return type_error_with_slice("Numbers or an iterable as arguments", unexpected)
            }
        };
        Ok(Num2(result))
    });

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.max(n.1)).into())),
        unexpected => num2_error(unexpected),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.min(n.1)).into())),
        unexpected => num2_error(unexpected),
    });

    result.add_fn("normalize", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Num2(n.normalize())),
        unexpected => num2_error(unexpected),
    });

    result.add_fn("product", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 * n.1).into())),
        unexpected => num2_error(unexpected),
    });

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 + n.1).into())),
        unexpected => num2_error(unexpected),
    });

    result.add_fn("with", |vm, args| match vm.get_args(args) {
        [Num2(n), Number(i), Number(value)] => {
            let mut result = *n;
            match usize::from(i) {
                0 => result.0 = value.into(),
                1 => result.1 = value.into(),
                other => return runtime_error!("Invalid index '{other}'"),
            }
            Ok(Num2(result))
        }
        unexpected => num2_error(unexpected),
    });

    result.add_fn("x", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number(n.0.into())),
        unexpected => num2_error(unexpected),
    });

    result.add_fn("y", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number(n.1.into())),
        unexpected => num2_error(unexpected),
    });

    result
}

fn num2_error(unexpected: &[Value]) -> RuntimeResult {
    type_error_with_slice("a Num2 as argument", unexpected)
}

pub(crate) fn num2_from_iterator(iterator: ValueIterator) -> Result<num2::Num2, RuntimeError> {
    let mut result = num2::Num2::default();
    for (i, value) in iterator.take(2).map(collect_pair).enumerate() {
        match value {
            Output::Value(Value::Number(n)) => result[i] = n.into(),
            Output::Value(unexpected) => return type_error_with_slice("a Number", &[unexpected]),
            Output::Error(e) => return Err(e),
            _ => unreachable!(), // ValuePairs collected in collect_pair
        }
    }
    Ok(result)
}
