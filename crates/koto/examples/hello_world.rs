use koto::prelude::*;

fn main() {
    Koto::default()
        .compile_and_run("print 'Hello, World!'")
        .unwrap();
}
