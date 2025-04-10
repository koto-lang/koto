use anyhow::{Result, bail};
use koto::prelude::*;

fn main() -> Result<()> {
    let script = "
export my_fn = |a, b| '{a} + {b} is {a + b}'
";
    let mut koto = Koto::default();

    // Run the script, which exports the `my_fn` function
    koto.compile_and_run(script)?;
    let Some(my_fn) = koto.exports().get("my_fn") else {
        bail!("Missing my_fn");
    };

    let call_result = koto.call_function(my_fn, &[1.into(), 2.into()])?;
    println!("Result: {}", koto.value_to_string(call_result)?);

    Ok(())
}
