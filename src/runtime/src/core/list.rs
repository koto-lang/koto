use {
    crate::{
        runtime_error,
        value_sort::{compare_values, sort_values},
        BinaryOp, Value, ValueIterator, ValueList, ValueMap,
    },
    std::cmp::Ordering,
    std::ops::DerefMut,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("clear", |vm, args| match vm.get_args(args) {
        [List(l)] => {
            l.data_mut().clear();
            Ok(Empty)
        }
        _ => runtime_error!("list.clear: Expected list as argument"),
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
        _ => runtime_error!("list.contains: Expected list and value as arguments"),
    });

    result.add_fn("copy", |vm, args| match vm.get_args(args) {
        [List(l)] => Ok(List(ValueList::with_data(l.data().clone()))),
        _ => runtime_error!("list.copy: Expected list as argument"),
    });

    result.add_fn("deep_copy", |vm, args| match vm.get_args(args) {
        [value @ List(_)] => Ok(value.deep_copy()),
        _ => runtime_error!("list.deep_copy: Expected list as argument"),
    });

    result.add_fn("fill", |vm, args| match vm.get_args(args) {
        [List(l), value] => {
            for v in l.data_mut().iter_mut() {
                *v = value.clone();
            }
            Ok(Empty)
        }
        _ => runtime_error!("list.fill: Expected list and value as arguments"),
    });

    result.add_fn("first", |vm, args| match vm.get_args(args) {
        [List(l)] => match l.data().first() {
            Some(value) => Ok(value.clone()),
            None => Ok(Empty),
        },
        _ => runtime_error!("list.first: Expected list as argument"),
    });

    result.add_fn("get", |vm, args| match vm.get_args(args) {
        [List(l), Number(n)] => {
            if *n < 0.0 {
                return runtime_error!("list.get: Negative indices aren't allowed");
            }
            match l.data().get(usize::from(n)) {
                Some(value) => Ok(value.clone()),
                None => Ok(Value::Empty),
            }
        }
        _ => runtime_error!("list.get: Expected list and number as arguments"),
    });

    result.add_fn("insert", |vm, args| match vm.get_args(args) {
        [List(l), Number(n), value] => {
            if *n < 0.0 {
                return runtime_error!("list.insert: Negative indices aren't allowed");
            }
            let index: usize = n.into();
            if index > l.data().len() {
                return runtime_error!("list.insert: Index out of bounds");
            }

            l.data_mut().insert(index, value.clone());
            Ok(Empty)
        }
        _ => runtime_error!("list.insert: Expected list, number, and value as arguments"),
    });

    result.add_fn("is_empty", |vm, args| match vm.get_args(args) {
        [List(l)] => Ok(Bool(l.data().is_empty())),
        _ => runtime_error!("list.is_empty: Expected list as argument"),
    });

    result.add_fn("iter", |vm, args| match vm.get_args(args) {
        [List(l)] => Ok(Iterator(ValueIterator::with_list(l.clone()))),
        _ => runtime_error!("list.iter: Expected list as argument"),
    });

    result.add_fn("last", |vm, args| match vm.get_args(args) {
        [List(l)] => match l.data().last() {
            Some(value) => Ok(value.clone()),
            None => Ok(Empty),
        },
        _ => runtime_error!("list.last: Expected list as argument"),
    });

    result.add_fn("pop", |vm, args| match vm.get_args(args) {
        [List(l)] => match l.data_mut().pop() {
            Some(value) => Ok(value),
            None => Ok(Empty),
        },
        _ => runtime_error!("list.pop: Expected list as argument"),
    });

    result.add_fn("push", |vm, args| match vm.get_args(args) {
        [List(l), value] => {
            l.data_mut().push(value.clone());
            Ok(Empty)
        }
        _ => runtime_error!("list.push: Expected list and value as arguments"),
    });

    result.add_fn("remove", |vm, args| match vm.get_args(args) {
        [List(l), Number(n)] => {
            if *n < 0.0 {
                return runtime_error!("list.remove: Negative indices aren't allowed");
            }
            let index: usize = n.into();
            if index >= l.data().len() {
                return runtime_error!(
                    "list.remove: Index out of bounds - \
                     the index is {} but the List only has {} elements",
                    index,
                    l.data().len(),
                );
            }

            Ok(l.data_mut().remove(index))
        }
        _ => runtime_error!("list.remove: Expected list and index as arguments"),
    });

    result.add_fn("resize", |vm, args| match vm.get_args(args) {
        [List(l), Number(n), value] => {
            if *n < 0.0 {
                return runtime_error!("list.resize: Negative sizes aren't allowed");
            }
            l.data_mut().resize(n.into(), value.clone());
            Ok(Empty)
        }
        _ => runtime_error!("list.resize: Expected list, number, and value as arguments"),
    });

    result.add_fn("retain", |vm, args| {
        match vm.get_args(args) {
            [List(l), f] if f.is_callable() => {
                let l = l.clone();
                let f = f.clone();

                let mut write_index = 0;
                for read_index in 0..l.len() {
                    let value = l.data()[read_index].clone();
                    match vm.run_function(f.clone(), &[value.clone()]) {
                        Ok(Bool(result)) => {
                            if result {
                                l.data_mut()[write_index] = value;
                                write_index += 1;
                            }
                        }
                        Ok(unexpected) => {
                            return runtime_error!(
                                "list.retain expects a Bool to be returned from the \
                                 predicate, found '{}'",
                                unexpected.type_as_string(),
                            );
                        }
                        Err(error) => return Err(error.with_prefix("list.retain")),
                    }
                }
                l.data_mut().resize(write_index, Empty);
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
                            error = Some(runtime_error!(
                                "list.retain:: Expected Bool from == comparison, found '{}'",
                                unexpected.type_as_string()
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
            }
            _ => {
                return runtime_error!(
                    "list.retain: Expected list and function or value as arguments"
                )
            }
        }

        Ok(Empty)
    });

    result.add_fn("reverse", |vm, args| match vm.get_args(args) {
        [List(l)] => {
            l.data_mut().reverse();
            Ok(Empty)
        }
        _ => runtime_error!("list.reverse: Expected list as argument"),
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [List(l)] => Ok(Number(l.len().into())),
        _ => runtime_error!("list.size: Expected list as argument"),
    });

    result.add_fn("sort", |vm, args| match vm.get_args(args) {
        [List(l)] => {
            let l = l.clone();
            let mut data = l.data_mut();
            sort_values(vm, &mut data)?;
            Ok(Empty)
        }
        [List(l), f] if f.is_callable() => {
            let l = l.clone();
            let f = f.clone();

            // apply function and construct a vec of (key, value)
            let mut pairs = l
                .data()
                .iter()
                .map(|value| match vm.run_function(f.clone(), &[value.clone()]) {
                    Ok(key) => Ok((key, value.clone())),
                    Err(e) => Err(e),
                })
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

            Ok(Empty)
        }
        _ => runtime_error!("list.sort: Expected list as argument"),
    });

    result.add_fn("sort_copy", |vm, args| match vm.get_args(args) {
        [List(l)] => {
            let mut result = l.data().clone();
            sort_values(vm, &mut result)?;
            Ok(List(ValueList::with_data(result)))
        }
        _ => runtime_error!("list.sort_copy: Expected list as argument"),
    });

    result.add_fn("swap", |vm, args| match vm.get_args(args) {
        [List(a), List(b)] => {
            std::mem::swap(a.data_mut().deref_mut(), b.data_mut().deref_mut());

            Ok(Empty)
        }
        _ => runtime_error!("list.swap: Expected two lists as arguments"),
    });

    result.add_fn("to_tuple", |vm, args| match vm.get_args(args) {
        [List(l)] => Ok(Value::Tuple(l.data().as_slice().into())),
        _ => runtime_error!("list.to_tuple expects a list as argument"),
    });

    result.add_fn("transform", |vm, args| match vm.get_args(args) {
        [List(l), f] if f.is_callable() => {
            let l = l.clone();
            let f = f.clone();

            for value in l.data_mut().iter_mut() {
                *value = match vm.run_function(f.clone(), &[value.clone()]) {
                    Ok(result) => result,
                    Err(error) => return Err(error.with_prefix("list.transform")),
                }
            }

            Ok(Empty)
        }
        _ => runtime_error!("list.transform expects a list and function as arguments"),
    });

    result.add_fn("with_size", |vm, args| match vm.get_args(args) {
        [Number(n), value] => {
            if *n < 0.0 {
                return runtime_error!("list.with_size: Negative sizes aren't allowed");
            }

            let result = smallvec::smallvec![value.clone(); n.into()];
            Ok(List(ValueList::with_data(result)))
        }
        _ => runtime_error!("list.with_size: Expected number and value as arguments"),
    });

    result
}
