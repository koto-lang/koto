#![allow(clippy::float_cmp)]

use koto::prelude::*;

#[test]
fn one_plus_two() {
    let mut koto = Koto::default();
    match koto.compile("1 + 2") {
        Ok(_) => match koto.run() {
            Ok(result) => match result {
                Value::Number(n) => assert_eq!(n, 3.0),
                other => panic!("Unexpected result: {other}"),
            },
            Err(runtime_error) => {
                panic!("Runtime error: {runtime_error}");
            }
        },
        Err(compiler_error) => {
            panic!("Compiler error: {compiler_error}");
        }
    }
}
