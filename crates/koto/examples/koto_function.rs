use anyhow::Result;
use koto::prelude::*;

fn main() -> Result<()> {
    let script = "
export foo = |a, b| '{a} + {b} is {a + b}'
";
    let mut koto = Koto::default();

    // Running the script exports the `foo` function
    koto.compile_and_run(script).unwrap();

    let result = koto.run_exported_function("foo", &[1.into(), 2.into()])?;
    println!("Result: {}", koto.value_to_string(result)?);

    Ok(())
}
