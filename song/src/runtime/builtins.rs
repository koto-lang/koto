use super::{Runtime, Value};
use std::rc::Rc;

pub fn register(runtime: &mut Runtime) {
    use Value::*;

    runtime.register_fn("assert", |args| {
        for value in args.iter() {
            match value {
                Bool(b) => {
                    if !b {
                        return Err("Assertion failed".to_string());
                    }
                }
                _ => return Err("assert only expects booleans as arguments".to_string()),
            }
        }
        Ok(Empty)
    });

    runtime.register_fn("assert_eq", |args| {
        if args.len() != 2 {
            Err(format!(
                "assert_eq expects two arguments, found {}",
                args.len()
            ))
        } else if args[0] == args[1] {
            Ok(Empty)
        } else {
            Err(format!(
                "Assertion failed, '{}' is not equal to '{}'",
                args[0], args[1]
            ))
        }
    });

    runtime.register_fn("assert_ne", |args| {
        if args.len() != 2 {
            Err(format!(
                "assert_ne expects two arguments, found {}",
                args.len()
            ))
        } else if args[0] != args[1] {
            Ok(Empty)
        } else {
            Err(format!(
                "Assertion failed, '{}' should not be equal to '{}'",
                args[0], args[1]
            ))
        }
    });

    runtime.register_fn("push", |args| {
        let mut arg_iter = args.iter();
        let first_arg_value = match arg_iter.next() {
            Some(arg) => arg,
            None => {
                return Err("Missing array as first argument for push".to_string());
            }
        };

        match first_arg_value {
            Array(array) => {
                let mut array = array.clone();
                let array_data = Rc::make_mut(&mut array);
                for value in arg_iter {
                    array_data.push(value.clone())
                }
                Ok(Array(array))
            }
            unexpected => {
                return Err(format!(
                    "push is only supported for arrays, found {}",
                    unexpected
                ))
            }
        }
    });

    runtime.register_fn("length", |args| {
        let mut arg_iter = args.iter();
        let first_arg_value = match arg_iter.next() {
            Some(arg) => arg,
            None => {
                return Err("Missing array as first argument for length".to_string());
            }
        };

        match first_arg_value {
            Array(array) => Ok(Number(array.len() as f64)),
            Range { min, max } => Ok(Number((max - min) as f64)),
            unexpected => Err(format!(
                "length is only supported for arrays and ranges, found {}",
                unexpected
            )),
        }
    });

    runtime.register_fn("print", |args| {
        for value in args.iter() {
            print!("{} ", value);
        }
        println!();
        Ok(Empty)
    });
}
