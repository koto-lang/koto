use anyhow::{bail, Result};
use koto::prelude::*;

fn main() -> Result<()> {
    let script = "1 + 2";

    let mut koto = Koto::default();
    match koto.compile_and_run(script)? {
        KValue::Number(result) => {
            println!("The result of '{script}' is {result}");
        }
        other => bail!(
            "Expected a Number, found '{}': ({})",
            other.type_as_string(),
            koto.value_to_string(other)?
        ),
    }

    Ok(())
}
