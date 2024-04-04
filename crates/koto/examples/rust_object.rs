use koto::{derive::*, prelude::*, Result};

fn main() {
    let script = "
foo = make_foo 41
print foo.get()
print foo.set 99
";
    let mut koto = Koto::default();
    koto.prelude().add_fn("make_foo", |ctx| match ctx.args() {
        [KValue::Number(n)] => Ok(Foo(n.into()).into()),
        unexpected => type_error_with_slice("a number", unexpected),
    });
    koto.compile_and_run(script).unwrap();
}

#[derive(Clone, Copy, KotoCopy, KotoType)]
struct Foo(i64);

// koto_impl generates Koto functions for any function tagged with #[koto_method]
#[koto_impl]
impl Foo {
    // A simple getter function
    #[koto_method]
    fn get(&self) -> Result<KValue> {
        Ok(self.0.into())
    }

    // A function that returns the object instance as the result
    #[koto_method]
    fn set(ctx: MethodContext<Self>) -> Result<KValue> {
        match ctx.args {
            [KValue::Number(n)] => {
                ctx.instance_mut()?.0 = n.into();
                ctx.instance_result()
            }
            unexpected => type_error_with_slice("a Number", unexpected),
        }
    }
}

impl KotoObject for Foo {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("{}({})", self.type_string(), self.0));
        Ok(())
    }
}

impl From<Foo> for KValue {
    fn from(x: Foo) -> Self {
        KObject::from(x).into()
    }
}
