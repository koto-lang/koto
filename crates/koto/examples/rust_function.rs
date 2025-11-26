use koto::{Result, prelude::*};

fn main() -> Result<()> {
    let mut koto = Koto::with_settings(KotoSettings::default().inherit_io());
    let prelude = koto.prelude();

    prelude.insert("say_hello", say_hello);
    prelude.insert("plus", plus);

    let script = "
say_hello()
say_hello 'Alice'

print plus 10, 20
";

    koto.compile_and_run(script)?;
    Ok(())
}

// The `koto_fn` macro generates wrappers for each function, with support for overloaded functions
koto_fn! {
    fn say_hello() {
        println!("Hello?");
    }

    fn say_hello(name: &str) {
        println!("Hello, {name}");
    }

    fn plus(a: i64, b: i64) -> i64 {
        a + b
    }
}
