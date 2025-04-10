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

    let exports = koto.exports();

    let Some(KValue::Number(exported_number)) = exports.get("number") else {
        bail!("Expected an exported number");
    };
    let Some(KValue::Str(exported_string)) = exports.get("string") else {
        bail!("Expected an exported string");
    };

    println!("Exported number: {exported_number}");
    println!("Exported string: '{exported_string}'");

    Ok(())
}
