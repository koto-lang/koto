use crate::{value, Value, ValueList, ValueMap};
use std::rc::Rc;

use crate::single_arg_fn;

pub fn register(global: &mut ValueMap) {
    use Value::*;

    let mut list = ValueMap::new();

    list.add_fn("add", |args| {
        let mut arg_iter = args.iter();
        let first_arg_value = match arg_iter.next() {
            Some(arg) => arg,
            None => {
                return Err("Missing list as first argument for list.add".to_string());
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
            unexpected => Err(format!(
                "list.add is only supported for lists, found {}",
                value::type_as_string(unexpected)
            )),
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
            Err("list.sort can only sort lists of numbers or strings".to_string())
        }
    });

    list.add_fn("fill", |args| {
        if args.len() != 2 {
            return Err(format!(
                "list.fill expects two arguments, found {}",
                args.len()
            ));
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
                        return Err(format!(
                            "list.fill expects a reference to a\
                                 list as its first argument, found {}",
                            value::type_as_string(&unexpected)
                        ))
                    }
                }
                Ok(Value::Empty)
            }
            unexpected => Err(format!(
                "list.fill expects a reference to a list as its first argument, found {}",
                value::type_as_string(unexpected)
            )),
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
