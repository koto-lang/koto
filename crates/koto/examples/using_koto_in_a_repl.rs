use koto::{Result, prelude::*};

fn main() -> Result<()> {
    let mut koto = Koto::default();

    // When using Koto in a REPL, variables from previous evaluations need to be made available
    // for the next evaluation.
    //
    // This is achieved by telling the compiler to treat each top-level assignment as if it had been
    // exported. i.e., the compiler turns the script `x = 1` into `export x = 1`.
    koto.compile_and_run(CompileArgs::new("x = 1").export_top_level_ids(true))?;
    assert!(koto.exports().get("x").is_some());

    // The exports map gets reused by the Koto instance for each run.
    match koto.compile_and_run(CompileArgs::new("x + x").export_top_level_ids(true))? {
        KValue::Number(result) => assert_eq!(result, KNumber::from(2)),
        unexpected => unexpected_type("Number", &unexpected)?,
    }

    Ok(())
}
