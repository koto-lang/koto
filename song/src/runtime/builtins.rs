use super::{Runtime, Value};
use std::rc::Rc;

pub fn register(runtime: &mut Runtime) {
    use Value::*;

    let builtins = runtime.builtins_mut();

    {
        let math = builtins.add_map("math");

        macro_rules! math_fn_1 {
            ($fn:ident) => {
                math_fn_1!(stringify!($fn), $fn)
            };
            ($name:expr, $fn:ident) => {
                math.add_fn($name, |args| {
                    if args.len() == 1 {
                        match args.first().unwrap() {
                            Number(n) => Ok(Number(n.$fn())),
                            unexpected => {
                                return Err(format!(
                                    "math.$fn only accepts a number as its argument, found {}",
                                    unexpected
                                ))
                            }
                        }
                    } else {
                        Err(format!(
                            "math.$fn expects one argument, found {}",
                            args.len()
                        ))
                    }
                });
            };
        }

        math_fn_1!(abs);
        math_fn_1!(acos);
        math_fn_1!(asin);
        math_fn_1!(atan);
        math_fn_1!(ceil);
        math_fn_1!(cos);
        math_fn_1!(cosh);
        math_fn_1!("degrees", to_degrees);
        math_fn_1!(exp);
        math_fn_1!(exp2);
        math_fn_1!(floor);
        math_fn_1!(log10);
        math_fn_1!(log2);
        math_fn_1!(ln);
        math_fn_1!("radians", to_radians);
        math_fn_1!(recip);
        math_fn_1!(sin);
        math_fn_1!(sinh);
        math_fn_1!(sqrt);
        math_fn_1!(tan);
        math_fn_1!(tanh);
    }

    {
        let array = builtins.add_map("array");

        array.add_fn("is_sortable", |args| {
            if args.len() == 1 {
                match args.first().unwrap() {
                    Array(a) => Ok(Bool(array_is_sortable(&a))),
                    unexpected => {
                        return Err(format!(
                            "array.sort only accepts an array as its argument, found {}",
                            unexpected
                        ))
                    }
                }
            } else {
                Err(format!(
                    "map.keys expects one argument, found {}",
                    args.len()
                ))
            }
        });

        array.add_fn("sort", |args| {
            if args.len() == 1 {
                match args.first().unwrap() {
                    Array(a) => {
                        if array_is_sortable(a.as_ref()) {
                            let mut a = Vec::clone(a);
                            a.sort();
                            Ok(Array(Rc::new(a)))
                        } else {
                            Err(format!(
                                "array.sort can only sort arrays of numbers or strings",
                            ))
                        }
                    }
                    unexpected => Err(format!(
                        "array.sort only accepts an array as its argument, found {}",
                        unexpected
                    )),
                }
            } else {
                Err(format!(
                    "array.sort expects one argument, found {}",
                    args.len()
                ))
            }
        });
    }

    {
        let map = builtins.add_map("map");
        map.add_fn("keys", |args| {
            if args.len() == 1 {
                match args.first().unwrap() {
                    Map(m) => Ok(Array(Rc::new(
                        m.as_ref()
                            .keys()
                            .map(|k| Str(k.clone()))
                            .collect::<Vec<_>>(),
                    ))),
                    unexpected => {
                        return Err(format!(
                            "map.keys only accepts maps as arguments, found {}",
                            unexpected
                        ))
                    }
                }
            } else {
                Err(format!(
                    "map.keys expects one argument, found {}",
                    args.len()
                ))
            }
        })
    }

    builtins.add_fn("assert", |args| {
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

    builtins.add_fn("assert_eq", |args| {
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

    builtins.add_fn("assert_ne", |args| {
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

    builtins.add_fn("push", |args| {
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

    builtins.add_fn("length", |args| {
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

    builtins.add_fn("print", |args| {
        for value in args.iter() {
            print!("{}", value);
        }
        println!();
        Ok(Empty)
    });
}

fn array_is_sortable(array: &Vec<Value>) -> bool {
    use Value::*;

    if array.len() == 0 {
        true
    } else {
        match array.first().unwrap() {
            value @ Number(_) | value @ Str(_) => {
                let value_type = std::mem::discriminant(value);
                array
                    .iter()
                    .all(|x| std::mem::discriminant(x) == value_type)
            }
            _ => false,
        }
    }
}
