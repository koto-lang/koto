use koto::{prelude::*, Result};

fn main() -> Result<()> {
    let script = "
export
  exported_a: '42'.to_number()
  exported_b: 'Hello from Koto'
";

    let mut koto = Koto::default();
    koto.compile_and_run(script)?;

    let exports = koto.exports();
    let exported_a = exports.get("exported_a").unwrap();
    let exported_b = exports.get("exported_b").unwrap();

    println!("exported_a: {}", koto.value_to_string(exported_a)?,);
    println!("exported_b: {}", koto.value_to_string(exported_b)?,);

    Ok(())
}
