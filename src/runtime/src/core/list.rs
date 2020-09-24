use crate::{external_error, value, RuntimeResult, Value, ValueList, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("contains", |_, args| match args {
        [List(l), value] => Ok(Bool(l.data().contains(value))),
        _ => external_error!("list.contains: Expected list and value as arguments"),
    });

    result.add_fn("fill", |_, args| {
        list_op(args, 2, "fill", |list| {
            let value = args[1].clone();
            for v in list.data_mut().iter_mut() {
                *v = value.clone();
            }
            Ok(Value::Empty)
        })
    });

    result.add_fn("filter", |runtime, args| {
        list_op(args, 2, "filter", |list| {
            match &args[1] {
                Function(f) => {
                    if f.arg_count != 1 {
                        return external_error!(
                            "The function passed to list.filter must have a \
                                         single argument, found '{}'",
                            f.arg_count,
                        );
                    }
                    let mut write_index = 0;
                    for read_index in 0..list.len() {
                        let value = list.data()[read_index].clone();
                        match runtime.run_function(f, &[value.clone()])? {
                            Bool(result) => {
                                if result {
                                    list.data_mut()[write_index] = value;
                                    write_index += 1;
                                }
                            }
                            unexpected => {
                                return external_error!(
                                    "list.filter expects a Bool to be returned from the \
                                     predicate, found '{}'",
                                    value::type_as_string(&unexpected),
                                );
                            }
                        }
                    }
                    list.data_mut().resize(write_index, Value::Empty);
                }
                value => {
                    list.data_mut().retain(|x| x == value);
                }
            };

            Ok(Value::Empty)
        })
    });

    result.add_fn("first", |_, args: &[Value]| {
        list_op(args, 1, "first", |list| match list.data().first() {
            Some(value) => Ok(value.clone()),
            None => Ok(Value::Empty),
        })
    });

    result.add_fn("fold", |runtime, args| {
        list_op(args, 3, "fold", |list| match &args {
            [_, result, Function(f)] => {
                if f.arg_count != 2 {
                    return external_error!(
                        "list.fold: The fold function must have two arguments, found '{}'",
                        f.arg_count,
                    );
                }

                let mut result = result.clone();
                for value in list.data().iter() {
                    result = runtime.run_function(f, &[result, value.clone()])?;
                }

                Ok(result)
            }
            [_, _, unexpected] => external_error!(
                "list.fold: Expected Function as third argument, found '{}'",
                value::type_as_string(&unexpected),
            ),
            _ => external_error!("list.fold: Expected initial value and function as arguments"),
        })
    });

    result.add_fn("get", |_, args: &[Value]| {
        list_op(args, 2, "get", |list| match &args[1] {
            Number(n) => {
                if *n < 0.0 {
                    return external_error!("list.get: Negative indices aren't allowed");
                }
                let index = *n as usize;
                match list.data().get(index) {
                    Some(value) => Ok(value.clone()),
                    None => Ok(Value::Empty),
                }
            }
            unexpected => external_error!(
                "list.get expects a number as its second argument, found '{}'",
                value::type_as_string(&unexpected),
            ),
        })
    });

    result.add_fn("insert", |_, args: &[Value]| {
        list_op(args, 3, "insert", |list| match &args[1] {
            Number(n) => {
                if *n < 0.0 {
                    return external_error!("list.insert: Negative indices aren't allowed");
                }
                let index = *n as usize;
                if index > list.data().len() {
                    return external_error!("list.insert: Index out of bounds");
                }

                list.data_mut().insert(index, args[2].clone());
                Ok(Value::Empty)
            }
            unexpected => external_error!(
                "list.insert expects a number as its second argument, found '{}'",
                value::type_as_string(&unexpected),
            ),
        })
    });

    result.add_fn("is_sortable", |_, args: &[Value]| {
        list_op(args, 1, "is_sortable", |list| {
            Ok(Bool(list_is_sortable(list)))
        })
    });

    result.add_fn("last", |_, args: &[Value]| {
        list_op(args, 1, "last", |list| match list.data().last() {
            Some(value) => Ok(value.clone()),
            None => Ok(Value::Empty),
        })
    });

    result.add_fn("length", |_, args: &[Value]| {
        list_op(args, 1, "is_sortable", |list| Ok(Number(list.len() as f64)))
    });

    result.add_fn("pop", |_, args: &[Value]| {
        list_op(args, 1, "pop", |list| match list.data_mut().pop() {
            Some(value) => Ok(value),
            None => Ok(Value::Empty),
        })
    });

    result.add_fn("push", |_, args: &[Value]| {
        list_op(args, 2, "push", |list| {
            list.data_mut().extend(args[1..].iter().cloned());
            Ok(Value::Empty)
        })
    });

    result.add_fn("remove", |_, args: &[Value]| {
        list_op(args, 2, "remove", |list| match &args[1] {
            Number(n) => {
                if *n < 0.0 {
                    return external_error!("list.remove: Negative indices aren't allowed");
                }
                let index = *n as usize;
                if index >= list.data().len() {
                    return external_error!(
                        "list.remove: Index out of bounds - \
                         the index is {} but the List only has {} elements",
                        index,
                        list.data().len(),
                    );
                }

                Ok(list.data_mut().remove(index))
            }
            unexpected => external_error!(
                "list.remove expects a number as its second argument, found '{}'",
                value::type_as_string(&unexpected),
            ),
        })
    });

    result.add_fn("reverse", |_, args: &[Value]| {
        list_op(args, 1, "sort", |list| {
            list.data_mut().reverse();
            Ok(Value::Empty)
        })
    });

    result.add_fn("sort", |_, args: &[Value]| {
        list_op(args, 1, "sort", |list| {
            list.data_mut().sort();
            Ok(Value::Empty)
        })
    });

    result.add_fn("sort_copy", |_, args: &[Value]| {
        list_op(args, 1, "sort_copy", |list| {
            if list_is_sortable(&list) {
                let mut result = list.data().clone();
                result.sort();
                Ok(List(ValueList::with_data(result)))
            } else {
                external_error!("list.sort_copy can only sort lists of numbers or strings")
            }
        })
    });

    result.add_fn("transform", |runtime, args| {
        list_op(args, 2, "transform", |list| match &args[1] {
            Function(f) => {
                if f.arg_count != 1 {
                    return external_error!(
                        "The function passed to list.transform must have a \
                                         single argument, found '{}'",
                        f.arg_count,
                    );
                }

                for value in list.data_mut().iter_mut() {
                    *value = runtime.run_function(f, &[value.clone()])?;
                }

                Ok(Value::Empty)
            }
            unexpected => external_error!(
                "list.transform expects a function as its second argument, found '{}'",
                value::type_as_string(&unexpected),
            ),
        })
    });

    result
}

fn list_op(
    args: &[Value],
    arg_count: usize,
    op_name: &str,
    mut op: impl FnMut(&ValueList) -> RuntimeResult,
) -> RuntimeResult {
    if args.len() < arg_count {
        return external_error!(
            "list.{} expects {} arguments, found {}",
            op_name,
            arg_count,
            args.len(),
        );
    }

    match &args[0] {
        Value::List(list) => op(&list),
        unexpected => external_error!(
            "list.{} expects a List as its first argument, found {}",
            op_name,
            value::type_as_string(&unexpected),
        ),
    }
}

fn list_is_sortable(list: &ValueList) -> bool {
    use Value::*;

    let data = list.data();

    if data.is_empty() {
        true
    } else {
        match data.first().unwrap() {
            value @ Number(_) | value @ Str(_) => {
                let value_type = std::mem::discriminant(value);
                data.iter().all(|x| std::mem::discriminant(x) == value_type)
            }
            _ => false,
        }
    }
}
