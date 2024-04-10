use koto::prelude::*;

fn main() {
    let script = "
print 'name: {name}'
print 'how_many: {how_many}'
print 'yes_or_no: {if yes_or_no then 'yes' else 'no'}'
";
    let mut koto = Koto::default();

    let prelude = koto.prelude();
    prelude.insert("name", "Alice");
    prelude.insert("how_many", 99);
    prelude.insert("yes_or_no", true);

    koto.compile_and_run(script).unwrap();
}
