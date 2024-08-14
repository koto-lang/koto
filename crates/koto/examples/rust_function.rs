use koto::prelude::*;

fn main() {
    let script = "
say_hello()
say_hello 'Alice'
print plus 10, 20
";
    let mut koto = Koto::default();
    let prelude = koto.prelude();

    // Standalone functions can be inserted directly
    prelude.insert("say_hello", say_hello);

    // The add_fn helper avoids the need for type annotations
    prelude.add_fn("plus", |ctx| match ctx.args() {
        [KValue::Number(a), KValue::Number(b)] => Ok((a + b).into()),
        unexpected => unexpected_args("|Number, Number|", unexpected),
    });

    koto.compile_and_run(script).unwrap();
}

fn say_hello(ctx: &mut CallContext) -> koto::Result<KValue> {
    match ctx.args() {
        [] => println!("Hello?"),
        [KValue::Str(name)] => println!("Hello, {name}"),
        unexpected => return unexpected_args("||, or |String|", unexpected),
    }

    Ok(KValue::Null)
}
