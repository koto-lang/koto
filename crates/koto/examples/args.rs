use koto::{Result, prelude::*};

fn main() -> Result<()> {
    let script = "
from os import args

if (size args) > 1
  for i, arg in args.enumerate()
    print '{i + 1}: {arg}'
else
  print 'No arguments'
";

    let mut koto = Koto::with_settings(KotoSettings::default().with_args(std::env::args()));

    koto.compile_and_run(script)?;

    Ok(())
}
