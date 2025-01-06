use anyhow::Result;
use koto::prelude::*;

fn main() -> Result<()> {
    let script = "
let x: String = 123
";
    let mut koto = Koto::default();

    // Type checks are enabled by default. Running the script will produce an error.
    let result = koto.compile_and_run(script);
    assert!(result.is_err());

    // Type checks can disabled via `CompileArgs::enable_type_checks`.
    // It should go without saying that checks should only be disabled if you're confident that your
    // code is correct!
    let result = koto.compile_and_run(CompileArgs::new(script).enable_type_checks(false));
    assert!(result.is_ok());

    Ok(())
}
