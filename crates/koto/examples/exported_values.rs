use koto::{prelude::*, Result};

fn main() -> Result<()> {
    let script = "
export
  foo: '42'.to_number()
  bar: 'Hello from Koto'
";

    let mut koto = Koto::default();
    koto.compile_and_run(script)?;

    let exports = koto.exports();
    let foo = exports.get("foo").unwrap();
    let bar = exports.get("bar").unwrap();

    println!("foo: {}", koto.value_to_string(foo)?,);
    println!("bar: {}", koto.value_to_string(bar)?,);

    Ok(())
}
