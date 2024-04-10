use koto::{derive::*, prelude::*, Result};

fn main() {
    let script = "
foo = make_foo 41
print foo.get()
print foo.set 99
";
    let mut koto = Koto::default();

    koto.prelude().add_fn("make_foo", |ctx| match ctx.args() {
        [KValue::Number(n)] => Ok(Foo::make_koto_object(*n).into()),
        unexpected => type_error_with_slice("a number", unexpected),
    });

    koto.compile_and_run(script).unwrap();
}

// Foo is a type that we want to use in Koto
//
// The KotoCopy and KotoType traits are automatically derived.
#[derive(Clone, Copy, KotoCopy, KotoType)]
struct Foo(i64);

// The KotoEntries trait is implemented by the koto_impl macro,
// generating Koto functions for any impl function tagged with #[koto_method],
// and inserting them into a cached KMap.
#[koto_impl]
impl Foo {
    fn make_koto_object(n: KNumber) -> KObject {
        // From is available for any type that implements KotoObject
        let foo = Self(n.into());
        KObject::from(foo)
    }

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
    // KotoObject::Display allows Foo to be used with Koto's print function
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("Foo({})", self.0));
        Ok(())
    }
}
