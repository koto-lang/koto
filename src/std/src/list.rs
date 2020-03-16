use crate::{builtin_error, single_arg_fn};
use koto_runtime::{value, value::deref_value, Error, RuntimeResult, Value, ValueList, ValueMap};
use std::rc::Rc;

pub fn register(global: &mut ValueMap) {
    use Value::*;

    let mut list = ValueMap::new();

    single_arg_fn!(list, "is_sortable", List, l, {
        Ok(Bool(list_is_sortable(&l)))
    });

    single_arg_fn!(list, "sort", List, l, {
        if list_is_sortable(l.as_ref()) {
            let mut result = Vec::clone(l.data());
            result.sort();
            Ok(List(Rc::new(ValueList::with_data(result))))
        } else {
            builtin_error!("list.sort can only sort lists of numbers or strings")
        }
    });

    list.add_fn("push", |_, args: &[Value]| {
        list_op(args, 2, "push", |list| {
            list.data_mut().extend(args[1..].iter().cloned());
            Ok(Value::Empty)
        })
    });

    list.add_fn("fill", |_, args| {
        list_op(args, 2, "fill", |list| {
            let value = args[1].clone();
            for v in list.data_mut().iter_mut() {
                *v = value.clone();
            }
            Ok(Value::Empty)
        })
    });

    list.add_fn("filter", |runtime, args| {
        list_op(args, 2, "filter", |list| {
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
        list_op(args, 2, "transform", |list| match &args[1] {
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
                "list.transform expects a function as its \
                                                   second argument, found '{}'",
                value::type_as_string(&unexpected)
            ),
        })
    });

    list.add_fn("fold", |runtime, args| {
        list_op(args, 3, "fold", |list| match &args[2] {
            Function(f) => {
                if f.args.len() != 2 {
                    return builtin_error!(
                        "The function passed to list.fold must have two \
                                     arguments, found '{}'",
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
                "list.transform expects a function as its \
                                                   second argument, found '{}'",
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
            Value::List(l) => op(Rc::make_mut(l)),
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
