use koto::{prelude::*, Result};

fn main() -> Result<()> {
    let script = "
export my_fn = |a, b| '{a} + {b} is {a + b}'
";
    let mut koto = Koto::default();

    // Running the script exports the `my_fn` function
    koto.compile_and_run(script).unwrap();
    let my_fn = koto.exports().get("my_fn").unwrap();
    assert!(my_fn.is_callable());

    let result = koto.call_function(my_fn, &[1.into(), 2.into()])?;
    println!("Result: {}", koto.value_to_string(result)?);

    Ok(())
}
