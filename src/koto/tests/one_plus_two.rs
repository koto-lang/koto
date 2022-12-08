use koto::prelude::*;

#[test]
fn one_plus_two() {
    let mut koto = Koto::default();

    if let Err(compiler_error) = koto.compile("1 + 2") {
        panic!("Compiler error: {compiler_error}");
    }

    match koto.run() {
        Ok(result) => match result {
            Value::Number(n) => assert_eq!(n, 3),
            other => panic!("Unexpected result: {other}"),
        },
        Err(runtime_error) => {
            panic!("Runtime error: {runtime_error}");
        }
    }
}
