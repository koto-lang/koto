fn main() {
    let script = "
if (size koto.args) > 0
  for i, arg in koto.args.enumerate()
    print '{i + 1}: {arg}'
else
  print 'No arguments'
";
    let mut koto = koto::Koto::default();
    let args: Vec<_> = std::env::args().skip(1).collect();
    koto.set_args(&args).unwrap();
    koto.compile_and_run(script).unwrap();
}
