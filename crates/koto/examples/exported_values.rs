use anyhow::{Result, bail};
use koto::prelude::*;

fn main() -> Result<()> {
    let script = "
export
  number: 42
  string: 'Hello from Koto'
";

    let mut koto = Koto::default();
    koto.compile_and_run(script)?;

    let exported_number = match koto.exports().get("number") {
        Some(KValue::Number(n)) => n,
        _ => bail!("Expected an exported number"),
    };
    let exported_string = match koto.exports().get("string") {
        Some(KValue::Str(s)) => s,
        _ => bail!("Expected an exported string"),
    };

    println!("Exported number: {exported_number}");
    println!("Exported string: '{exported_string}'");

    Ok(())
}
