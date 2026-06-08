use anyhow::{Result, bail};
use koto::prelude::*;

fn main() -> Result<()> {
    let script = "
export x = a + b
";
    let mut koto = Koto::with_settings(KotoSettings::default().inherit_io());
    koto.prelude().insert("a", 3);
    koto.prelude().insert("b", 4);
    koto.compile_and_run(script).unwrap();

    let Some(KValue::Number(x1)) = koto.exports().get("x") else {
        bail!("Expected 7");
    };

    // Spawn a Koto instance that uses shared resources.
    let mut koto2 = koto.spawn_shared();
    // `koto2` shares the same prelude as `koto` (here `a` is replaced while `b` is left alone).
    koto2.prelude().insert("a", 10);
    // ...although the spawned runtime is given a separate exports map.
    assert!(koto2.exports().is_empty());

    koto2.compile_and_run(script).unwrap();
    let Some(KValue::Number(x2)) = koto2.exports().get("x") else {
        bail!("Expected 14");
    };

    println!("x1: {x1}");
    println!("x2: {x2}");

    Ok(())
}
