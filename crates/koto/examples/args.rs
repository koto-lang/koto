use koto::prelude::*;

fn main() {
    let script = "
from koto import args

if (size args) > 0
  for i, arg in args.enumerate()
    print '{i + 1}: {arg}'
else
  print 'No arguments'
";
    let mut koto = Koto::default();
    let args: Vec<_> = std::env::args().collect();
    koto.set_args(&args).unwrap();
    koto.compile_and_run(script).unwrap();
}
