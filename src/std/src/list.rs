use crate::{builtin_error, single_arg_fn};
use koto_runtime::{
    value, value::deref_value, Error, RuntimeResult, Value, ValueList, ValueMap, ValueVec,
};
use std::{cell::RefCell, rc::Rc};

pub fn register(global: &mut ValueMap) {
    use Value::*;

    let mut list = ValueMap::new();

    single_arg_fn!(list, "is_sortable", List, l, {
        Ok(Bool(list_is_sortable(&l.borrow())))
    });

    single_arg_fn!(list, "sort_copy", List, l, {
        if list_is_sortable(&l.borrow()) {
            let mut result = ValueVec::clone(l.borrow().data());
            result.sort();
            Ok(List(Rc::new(RefCell::new(ValueList::with_data(result)))))
        } else {
            builtin_error!("list.sort_copy can only sort lists of numbers or strings")
        }
    });

    list.add_fn("sort", |_, args: &[Value]| {
        ref_list_op(args, 1, "sort", |list| {
            list.data_mut().sort();
            Ok(Value::Empty)
        })
    });

    list.add_fn("push", |_, args: &[Value]| {
        ref_list_op(args, 2, "push", |list| {
            list.data_mut().extend(args[1..].iter().cloned());
            Ok(Value::Empty)
        })
    });

    list.add_fn("pop", |_, args: &[Value]| {
        ref_list_op(args, 1, "pop", |list| match list.data_mut().pop() {
            Some(value) => Ok(value),
            None => Ok(Value::Empty),
        })
    });

    list.add_fn("get", |_, args: &[Value]| {
        list_op(args, 2, "get", |list| match &args[1] {
            Number(n) => {
                if *n < 0.0 {
                    return builtin_error!("list.get: Negative indices aren't allowed");
                }
                let index = *n as usize;
                match list.data().get(index) {
                    Some(value) => Ok(value.clone()),
                    None => Ok(Value::Empty),
                }
            }
            unexpected => builtin_error!(
                "list.get expects a number as its second argument, found '{}'",
                value::type_as_string(&unexpected)
            ),
        })
    });

    list.add_fn("remove", |_, args: &[Value]| {
        ref_list_op(args, 2, "remove", |list| match &args[1] {
            Number(n) => {
                if *n < 0.0 {
                    return builtin_error!("list.remove: Negative indices aren't allowed");
                }
                let index = *n as usize;
                if index >= list.data().len() {
                    return builtin_error!(
                        "list.remove: Index out of bounds - \
                         the index is {} but the List only has {} elements",
                        index,
                        list.data().len()
                    );
                }

                Ok(list.data_mut().remove(index))
            }
            unexpected => builtin_error!(
                "list.remove expects a number as its second argument, found '{}'",
                value::type_as_string(&unexpected)
            ),
        })
    });

    list.add_fn("insert", |_, args: &[Value]| {
        ref_list_op(args, 3, "insert", |list| match &args[1] {
            Number(n) => {
                if *n < 0.0 {
                    return builtin_error!("list.insert: Negative indices aren't allowed");
                }
                let index = *n as usize;
                if index > list.data().len() {
                    return builtin_error!("list.insert: Index out of bounds");
                }

                list.data_mut().insert(index, args[2].clone());
                Ok(Value::Empty)
            }
            unexpected => builtin_error!(
                "list.insert expects a number as its second argument, found '{}'",
                value::type_as_string(&unexpected)
            ),
        })
    });

    list.add_fn("fill", |_, args| {
        ref_list_op(args, 2, "fill", |list| {
            let value = args[1].clone();
            for v in list.data_mut().iter_mut() {
                *v = value.clone();
            }
            Ok(Value::Empty)
        })
    });

    list.add_fn("filter", |runtime, args| {
        ref_list_op(args, 2, "filter", |list| {
            match &args[1] {
                Function(f) => {
                    if f.args.len() != 1 {
                        return builtin_error!(
                            "The function passed to list.filter must have a \
                                         single argument, found '{}'",
                            f.args.len()
                        );
                    }
                    let mut write_index = 0;
                    for read_index in 0..list.data().len() {
                        let value = list.data()[read_index].clone();
                        match runtime.call_function(f, &[value.clone()])? {
                            Bool(result) => {
                                if result {
                                    list.data_mut()[write_index] = value;
                                    write_index += 1;
                                }
                            }
                            unexpected => {
                                return builtin_error!(
                                    "list.filter expects a Bool to be returned from the \
                                                    predicate, found '{}'",
                                    value::type_as_string(&unexpected)
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

    list.add_fn("transform", |runtime, args| {
        ref_list_op(args, 2, "transform", |list| match &args[1] {
            Function(f) => {
                if f.args.len() != 1 {
                    return builtin_error!(
                        "The function passed to list.transform must have a \
                                         single argument, found '{}'",
                        f.args.len()
                    );
                }

                for value in list.data_mut().iter_mut() {
                    *value = runtime.call_function(f, &[value.clone()])?;
                }

                Ok(Value::Empty)
            }
            unexpected => builtin_error!(
                "list.transform expects a function as its second argument, found '{}'",
                value::type_as_string(&unexpected)
            ),
        })
    });

    list.add_fn("fold", |runtime, args| {
        list_op(args, 3, "fold", |list| match &args[2] {
            Function(f) => {
                if f.args.len() != 2 {
                    return builtin_error!(
                        "The function passed to list.fold must have two arguments, found '{}'",
                        f.args.len()
                    );
                }

                let mut result = args[1].clone();
                for value in list.data().iter() {
                    result = runtime.call_function(f, &[result, value.clone()])?;
                }

                Ok(result)
            }
            unexpected => builtin_error!(
                "list.transform expects a function as its second argument, found '{}'",
                value::type_as_string(&unexpected)
            ),
        })
    });

    global.add_map("list", list);
}

fn list_op<'a>(
    args: &[Value<'a>],
    arg_count: usize,
    op_name: &str,
    mut op: impl FnMut(&ValueList<'a>) -> RuntimeResult<'a>,
) -> RuntimeResult<'a> {
    if args.len() < arg_count {
        return builtin_error!(
            "list.{} expects {} arguments, found {}",
            op_name,
            arg_count,
            args.len()
        );
    }

    match deref_value(&args[0]) {
        Value::List(list) => op(&list.borrow()),
        unexpected => builtin_error!(
            "list.{} expects a List as its first argument, found {}",
            op_name,
            value::type_as_string(&unexpected)
        ),
    }
}

fn ref_list_op<'a>(
    args: &[Value<'a>],
    arg_count: usize,
    op_name: &str,
    mut op: impl FnMut(&mut ValueList<'a>) -> RuntimeResult<'a>,
) -> RuntimeResult<'a> {
    if args.len() < arg_count {
        return builtin_error!(
            "list.{} expects {} arguments, found {}",
            op_name,
            arg_count,
            args.len()
        );
    }

    match &args[0] {
        Value::Ref(r) => match &mut *r.borrow_mut() {
            Value::List(l) => op(&mut l.borrow_mut()),
            unexpected => builtin_error!(
                "list.{} expects a reference to a list as its first argument, found {}",
                op_name,
                value::type_as_string(&unexpected)
            ),
        },
        unexpected => builtin_error!(
            "list.{} expects a reference to a list as its first argument, found {}",
            op_name,
            value::type_as_string(unexpected)
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
