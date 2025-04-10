use koto::{Result, prelude::*};

fn main() -> Result<()> {
    let script = "print 'Hello, World!'";

    Koto::default().compile_and_run(script)?;

    Ok(())
}
