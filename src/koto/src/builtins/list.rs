use crate::{
    builtin_error, single_arg_fn, value, value::deref_value, Error, Value, ValueList, ValueMap,
};
use std::rc::Rc;

pub fn register(global: &mut ValueMap) {
    use Value::*;

    let mut list = ValueMap::new();

    list.add_fn("add", |_, args: &[Value]| {
        let mut arg_iter = args.iter();
        let first_arg_value = match arg_iter.next() {
            Some(arg) => arg,
            None => {
                return builtin_error!("Missing list as first argument for list.add");
            }
        };

        match first_arg_value {
            List(list) => {
                let mut list = list.clone();
                let list_data = Rc::make_mut(&mut list).data_mut();
                for value in arg_iter {
                    list_data.push(value.clone())
                }
                Ok(List(list))
            }
            unexpected => builtin_error!(
                "list.add is only supported for lists, found {}",
                value::type_as_string(unexpected)
            ),
        }
    });

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

    list.add_fn("fill", |_, args| {
        if args.len() != 2 {
            return builtin_error!("list.fill expects two arguments, found {}", args.len());
        }

        match &args[0] {
            Ref(r) => {
                match &mut *r.borrow_mut() {
                    List(l) => {
                        let value = args[1].clone();
                        for v in Rc::make_mut(l).data_mut().iter_mut() {
                            *v = value.clone();
                        }
                    }
                    unexpected => {
                        return builtin_error!(
                            "list.fill expects a reference to a\
                                 list as its first argument, found {}",
                            value::type_as_string(&unexpected)
                        )
                    }
                }
                Ok(Value::Empty)
            }
            unexpected => builtin_error!(
                "list.fill expects a reference to a list as its first argument, found {}",
                value::type_as_string(unexpected)
            ),
        }
    });

    list.add_fn("filter", |runtime, args| {
        if args.len() != 2 {
            return builtin_error!("list.filter expects two arguments, found {}", args.len());
        }

        match &args[0] {
            Ref(r) => {
                match &mut *r.borrow_mut() {
                    List(l) => match &args[1] {
                        Function(f) => {
                            if f.args.len() != 1 {
                                return builtin_error!(
                                    "The function passed to list.filter must have a \
                                         single argument, found '{}'",
                                    f.args.len()
                                );
                            }
                            let mut write_index = 0;
                            for read_index in 0..l.data().len() {
                                let value = l.data()[read_index].clone();
                                match runtime.call_function(f, &[value.clone()])? {
                                    Bool(result) => {
                                        if result {
                                            Rc::make_mut(l).data_mut()[write_index] = value;
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
                            Rc::make_mut(l).data_mut().resize(write_index, Value::Empty);
                        }
                        value => {
                            Rc::make_mut(l).data_mut().retain(|x| x == value);
                        }
                    },
                    unexpected => {
                        return builtin_error!(
                            "list.filter expects a reference to a\
                                 list as its first argument, found {}",
                            value::type_as_string(&unexpected)
                        )
                    }
                }

                Ok(Value::Empty)
            }
            unexpected => builtin_error!(
                "list.filter expects a reference to a list as its first argument, found {}",
                value::type_as_string(unexpected)
            ),
        }
    });

    list.add_fn("transform", |runtime, args| {
        if args.len() != 2 {
            return builtin_error!("list.transform expects two arguments, found {}", args.len());
        }

        match &args[0] {
            Ref(r) => {
                match &mut *r.borrow_mut() {
                    List(l) => match &args[1] {
                        Function(f) => {
                            if f.args.len() != 1 {
                                return builtin_error!(
                                    "The function passed to list.transform must have a \
                                         single argument, found '{}'",
                                    f.args.len()
                                );
                            }

                            for value in Rc::make_mut(l).data_mut().iter_mut() {
                                *value = runtime.call_function(f, &[value.clone()])?;
                            }
                        }
                        unexpected => {
                            return builtin_error!(
                                "list.transform expects a function as its \
                                                   second argument, found '{}'",
                                value::type_as_string(&unexpected)
                            )
                        }
                    },
                    unexpected => {
                        return builtin_error!(
                            "list.transform expects a reference to a\
                                 list as its first argument, found {}",
                            value::type_as_string(&unexpected)
                        )
                    }
                }

                Ok(Value::Empty)
            }
            unexpected => builtin_error!(
                "list.transform expects a reference to a list as its first argument, found {}",
                value::type_as_string(unexpected)
            ),
        }
    });

    list.add_fn("fold", |runtime, args| {
        if args.len() != 3 {
            return builtin_error!("list.fold expects three arguments, found {}", args.len());
        }

        match &args[0] {
            Ref(r) => {
                match &mut *r.borrow_mut() {
                    List(l) => match &args[2] {
                        Function(f) => {
                            if f.args.len() != 2 {
                                return builtin_error!(
                                    "The function passed to list.fold must have two \
                                     arguments, found '{}'",
                                    f.args.len()
                                );
                            }

                            let mut result = args[1].clone();
                            for value in l.data().iter() {
                                result = runtime.call_function(f, &[result, value.clone()])?;
                            }

                            Ok(result)
                        }
                        unexpected => {
                            builtin_error!(
                                "list.transform expects a function as its \
                                                   second argument, found '{}'",
                                value::type_as_string(&unexpected)
                            )
                        }
                    },
                    unexpected => {
                        builtin_error!(
                            "list.fold expects a reference to a\
                                 list as its first argument, found {}",
                            value::type_as_string(&unexpected)
                        )
                    }
                }
            }
            unexpected => builtin_error!(
                "list.fold expects a reference to a list as its first argument, found {}",
                value::type_as_string(unexpected)
            ),
        }
    });

    global.add_map("list", list);
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
