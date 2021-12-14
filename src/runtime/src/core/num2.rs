use {
    super::iterator::collect_pair,
    crate::{
        num2, unexpected_type_error_with_slice,
        value_iterator::{make_iterator, ValueIteratorOutput as Output},
        RuntimeResult, Value, ValueMap,
    },
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("length", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number(n.length().into())),
        unexpected => num2_error("length", unexpected),
    });

    result.add_fn("make_num2", |vm, args| {
        let result = match vm.get_args(args) {
            [Number(n)] => num2::Num2(n.into(), n.into()),
            [Number(n1), Number(n2)] => num2::Num2(n1.into(), n2.into()),
            [Num2(n)] => *n,
            [iterable] if iterable.is_iterable() => {
                let iterator = make_iterator(iterable).unwrap();
                let mut result = num2::Num2::default();
                for (i, value) in iterator.take(2).map(collect_pair).enumerate() {
                    match value {
                        Output::Value(Number(n)) => result[i] = n.into(),
                        Output::Value(unexpected) => {
                            return unexpected_type_error_with_slice(
                                "num2.make_num2",
                                "a Number",
                                &[unexpected],
                            )
                        }
                        Output::Error(e) => return Err(e),
                        _ => unreachable!(), // ValuePairs collected in collect_pair
                    }
                }
                result
            }
            unexpected => {
                return unexpected_type_error_with_slice(
                    "num2.make_num2",
                    "Numbers or an iterable as arguments",
                    unexpected,
                )
            }
        };
        Ok(Num2(result))
    });

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.max(n.1)).into())),
        unexpected => num2_error("max", unexpected),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.min(n.1)).into())),
        unexpected => num2_error("min", unexpected),
    });

    result.add_fn("normalize", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Num2(n.normalize())),
        unexpected => num2_error("normalize", unexpected),
    });

    result.add_fn("product", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 * n.1).into())),
        unexpected => num2_error("product", unexpected),
    });

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 + n.1).into())),
        unexpected => num2_error("sum", unexpected),
    });

    result
}

fn num2_error(name: &str, unexpected: &[Value]) -> RuntimeResult {
    unexpected_type_error_with_slice(&format!("num2.{}", name), "a Num2 as argument", unexpected)
}
