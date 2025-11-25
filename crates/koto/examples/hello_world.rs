use koto::{Result, prelude::*};

fn main() -> Result<()> {
    let script = "print 'Hello, World!'";

    Koto::with_settings(KotoSettings::default().inherit_io()).compile_and_run(script)?;

    Ok(())
}
