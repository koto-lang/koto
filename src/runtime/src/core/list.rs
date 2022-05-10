use {
    crate::{
        runtime_error, unexpected_type_error_with_slice,
        value_sort::{compare_values, sort_values},
        BinaryOp, CallArgs, Value, ValueList, ValueMap,
    },
    std::{cmp::Ordering, ops::DerefMut},
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::new();

    result.add_fn("clear", |vm, args| match vm.get_args(args) {
        [List(l)] => {
            l.data_mut().clear();
            Ok(List(l.clone()))
        }
        unexpected => {
            unexpected_type_error_with_slice("list.clear", "a List as argument", unexpected)
        }
    });

    result.add_fn("contains", |vm, args| match vm.get_args(args) {
        [List(l), value] => {
            let l = l.clone();
            let value = value.clone();
            for candidate in l.data().iter() {
                match vm.run_binary_op(BinaryOp::Equal, value.clone(), candidate.clone()) {
                    Ok(Bool(false)) => {}
                    Ok(Bool(true)) => return Ok(true.into()),
                    Ok(unexpected) => {
                        return runtime_error!(
                            "list.contains: Expected Bool from comparison, found '{}'",
                            unexpected.type_as_string()
                        )
                    }
                    Err(e) => return Err(e.with_prefix("list.contains")),
                }
            }
            Ok(false.into())
        }
        unexpected => unexpected_type_error_with_slice(
            "list.contains",
            "a List and Value as arguments",
            unexpected,
        ),
    });

    result.add_fn("copy", |vm, args| match vm.get_args(args) {
        [List(l)] => Ok(List(ValueList::with_data(l.data().clone()))),
        unexpected => {
            unexpected_type_error_with_slice("list.copy", "a List as argument", unexpected)
        }
    });

    result.add_fn("deep_copy", |vm, args| match vm.get_args(args) {
        [value @ List(_)] => Ok(value.deep_copy()),
        unexpected => {
            unexpected_type_error_with_slice("list.deep_copy", "a List as argument", unexpected)
        }
    });

    result.add_fn("fill", |vm, args| match vm.get_args(args) {
        [List(l), value] => {
            for v in l.data_mut().iter_mut() {
                *v = value.clone();
            }
            Ok(List(l.clone()))
        }
        unexpected => unexpected_type_error_with_slice(
            "list.fill",
            "a List and Value as arguments",
            unexpected,
        ),
    });

    result.add_fn("first", |vm, args| match vm.get_args(args) {
        [List(l)] => match l.data().first() {
            Some(value) => Ok(value.clone()),
            None => Ok(Null),
        },
        unexpected => {
            unexpected_type_error_with_slice("list.first", "a List as argument", unexpected)
        }
    });

    result.add_fn("get", |vm, args| {
        let (list, index, default) = match vm.get_args(args) {
            [List(list), Number(n)] => (list, n, &Null),
            [List(list), Number(n), default] => (list, n, default),
            unexpected => {
                return unexpected_type_error_with_slice(
                    "list.get",
                    "a List and a Number (with optional default value) as arguments",
                    unexpected,
                )
            }
        };

        match list.data().get::<usize>(index.into()) {
            Some(value) => Ok(value.clone()),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("insert", |vm, args| match vm.get_args(args) {
        [List(l), Number(n), value] if *n >= 0.0 => {
            let index: usize = n.into();
            if index > l.data().len() {
                return runtime_error!("list.insert: Index out of bounds");
            }

            l.data_mut().insert(index, value.clone());
            Ok(List(l.clone()))
        }
        unexpected => unexpected_type_error_with_slice(
            "list.insert",
            "a List, a non-negative Number, and Value as arguments",
            unexpected,
        ),
    });

    result.add_fn("is_empty", |vm, args| match vm.get_args(args) {
        [List(l)] => Ok(Bool(l.data().is_empty())),
        unexpected => {
            unexpected_type_error_with_slice("list.is_empty", "a List as argument", unexpected)
        }
    });

    result.add_fn("last", |vm, args| match vm.get_args(args) {
        [List(l)] => match l.data().last() {
            Some(value) => Ok(value.clone()),
            None => Ok(Null),
        },
        unexpected => {
            unexpected_type_error_with_slice("list.last", "a List as argument", unexpected)
        }
    });

    result.add_fn("pop", |vm, args| match vm.get_args(args) {
        [List(l)] => match l.data_mut().pop() {
            Some(value) => Ok(value),
            None => Ok(Null),
        },
        unexpected => {
            unexpected_type_error_with_slice("list.pop", "a List as argument", unexpected)
        }
    });

    result.add_fn("push", |vm, args| match vm.get_args(args) {
        [List(l), value] => {
            l.data_mut().push(value.clone());
            Ok(List(l.clone()))
        }
        unexpected => unexpected_type_error_with_slice(
            "list.push",
            "a List and Value as arguments",
            unexpected,
        ),
    });

    result.add_fn("remove", |vm, args| match vm.get_args(args) {
        [List(l), Number(n)] if *n >= 0.0 => {
            let index: usize = n.into();
            if index >= l.data().len() {
                return runtime_error!(
                    "list.remove: Index out of bounds - \
                     the index is {index} but the List only has {} elements",
                    l.data().len(),
                );
            }

            Ok(l.data_mut().remove(index))
        }
        unexpected => unexpected_type_error_with_slice(
            "list.remove",
            "a List and non-negative Number as arguments",
            unexpected,
        ),
    });

    result.add_fn("resize", |vm, args| match vm.get_args(args) {
        [List(l), Number(n)] if *n >= 0.0 => {
            l.data_mut().resize(n.into(), Null);
            Ok(List(l.clone()))
        }
        [List(l), Number(n), value] if *n >= 0.0 => {
            l.data_mut().resize(n.into(), value.clone());
            Ok(List(l.clone()))
        }
        unexpected => unexpected_type_error_with_slice(
            "list.resize",
            "a List, a non-negative Number, and optional Value as arguments",
            unexpected,
        ),
    });

    result.add_fn("resize_with", |vm, args| match vm.get_args(args) {
        [List(l), Number(n), f] if *n >= 0.0 && f.is_callable() => {
            let new_size = usize::from(n);
            let len = l.len();
            let l = l.clone();
            let f = f.clone();

            match len.cmp(&new_size) {
                Ordering::Greater => l.data_mut().truncate(new_size),
                Ordering::Less => {
                    l.data_mut().reserve(new_size);
                    for _ in 0..new_size - len {
                        let new_value = vm.run_function(f.clone(), CallArgs::None)?;
                        l.data_mut().push(new_value);
                    }
                }
                Ordering::Equal => {}
            }

            Ok(List(l))
        }
        unexpected => unexpected_type_error_with_slice(
            "list.resize_with",
            "a List, a non-negative Number, and Function as arguments",
            unexpected,
        ),
    });

    result.add_fn("retain", |vm, args| {
        let result = match vm.get_args(args) {
            [List(l), f] if f.is_callable() => {
                let l = l.clone();
                let f = f.clone();

                let mut write_index = 0;
                for read_index in 0..l.len() {
                    let value = l.data()[read_index].clone();
                    match vm.run_function(f.clone(), CallArgs::Single(value.clone())) {
                        Ok(Bool(result)) => {
                            if result {
                                l.data_mut()[write_index] = value;
                                write_index += 1;
                            }
                        }
                        Ok(unexpected) => {
                            return unexpected_type_error_with_slice(
                                "list.retain",
                                "a Bool to returned from the predicate",
                                &[unexpected],
                            );
                        }
                        Err(error) => return Err(error.with_prefix("list.retain")),
                    }
                }
                l.data_mut().resize(write_index, Null);
                l
            }
            [List(l), value] => {
                let l = l.clone();
                let value = value.clone();

                let mut error = None;
                l.data_mut().retain(|x| {
                    if error.is_some() {
                        return true;
                    }
                    match vm.run_binary_op(BinaryOp::Equal, x.clone(), value.clone()) {
                        Ok(Bool(true)) => true,
                        Ok(Bool(false)) => false,
                        Ok(unexpected) => {
                            error = Some(unexpected_type_error_with_slice(
                                "list.retain",
                                "a Bool from the equality comparison",
                                &[unexpected],
                            ));
                            true
                        }
                        Err(e) => {
                            error = Some(Err(e.with_prefix("list.retain")));
                            true
                        }
                    }
                });
                if let Some(error) = error {
                    return error;
                }
                l
            }
            unexpected => {
                return unexpected_type_error_with_slice(
                    "list.retain",
                    "a List and either a predicate Function or Value as arguments",
                    unexpected,
                )
            }
        };

        Ok(List(result))
    });

    result.add_fn("reverse", |vm, args| match vm.get_args(args) {
        [List(l)] => {
            l.data_mut().reverse();
            Ok(List(l.clone()))
        }
        unexpected => {
            unexpected_type_error_with_slice("list.reverse", "a List as argument", unexpected)
        }
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [List(l)] => Ok(Number(l.len().into())),
        unexpected => {
            unexpected_type_error_with_slice("list.size", "a List as argument", unexpected)
        }
    });

    result.add_fn("sort", |vm, args| match vm.get_args(args) {
        [List(l)] => {
            let l = l.clone();
            let mut data = l.data_mut();
            sort_values(vm, &mut data)?;
            Ok(List(l.clone()))
        }
        [List(l), f] if f.is_callable() => {
            let l = l.clone();
            let f = f.clone();

            // apply function and construct a vec of (key, value)
            let mut pairs = l
                .data()
                .iter()
                .map(
                    |value| match vm.run_function(f.clone(), CallArgs::Single(value.clone())) {
                        Ok(key) => Ok((key, value.clone())),
                        Err(e) => Err(e),
                    },
                )
                .collect::<Result<Vec<_>, _>>()?;

            let mut error = None;

            // sort array by key (i.e. from [key, value])
            pairs.sort_by(|a, b| {
                if error.is_some() {
                    return Ordering::Equal;
                }

                match compare_values(vm, &a.0, &b.0) {
                    Ok(ordering) => ordering,
                    Err(e) => {
                        error.get_or_insert(e);
                        Ordering::Equal
                    }
                }
            });

            if let Some(error) = error {
                return Err(error.with_prefix("list.sort"));
            }

            // collect values
            *l.data_mut() = pairs
                .iter()
                .map(|(_key, value)| value.clone())
                .collect::<_>();

            Ok(List(l))
        }
        unexpected => {
            unexpected_type_error_with_slice("list.sort", "a List as argument", unexpected)
        }
    });

    result.add_fn("swap", |vm, args| match vm.get_args(args) {
        [List(a), List(b)] => {
            std::mem::swap(a.data_mut().deref_mut(), b.data_mut().deref_mut());
            Ok(Null)
        }
        unexpected => {
            unexpected_type_error_with_slice("list.swap", "two Lists as arguments", unexpected)
        }
    });

    result.add_fn("to_tuple", |vm, args| match vm.get_args(args) {
        [List(l)] => Ok(Value::Tuple(l.data().as_slice().into())),
        unexpected => {
            unexpected_type_error_with_slice("list.to_tuple", "a List as argument", unexpected)
        }
    });

    result.add_fn("transform", |vm, args| match vm.get_args(args) {
        [List(l), f] if f.is_callable() => {
            let l = l.clone();
            let f = f.clone();

            for value in l.data_mut().iter_mut() {
                *value = match vm.run_function(f.clone(), CallArgs::Single(value.clone())) {
                    Ok(result) => result,
                    Err(error) => return Err(error.with_prefix("list.transform")),
                }
            }

            Ok(List(l))
        }
        unexpected => unexpected_type_error_with_slice(
            "list.transform",
            "a List and Function as arguments",
            unexpected,
        ),
    });

    result.add_fn("with_size", |vm, args| match vm.get_args(args) {
        [Number(n), value] if *n >= 0.0 => {
            let result = smallvec::smallvec![value.clone(); n.into()];
            Ok(List(ValueList::with_data(result)))
        }
        unexpected => unexpected_type_error_with_slice(
            "list.with_size",
            "a non-negative Number and Value as arguments",
            unexpected,
        ),
    });

    result
}
