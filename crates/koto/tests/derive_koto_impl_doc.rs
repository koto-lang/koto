use koto::{derive::*, prelude::*, runtime::Result};

#[allow(unused_variables, dead_code)]
mod snippets {
    use super::*;

    mod access {
        use super::*;

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access]
                fn foo(&self) -> KValue {
                    unimplemented!()
                }
            }
        };

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access]
                fn foo(&self) -> Result<KValue> {
                    unimplemented!()
                }
            }
        };
    }

    mod access_assign {
        use super::*;

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_assign]
                fn set_foo(&mut self, value: &KValue) {
                    unimplemented!()
                }
            }
        };

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_assign]
                fn set_foo(&mut self, value: &KValue) -> Result<()> {
                    unimplemented!()
                }
            }
        };
    }

    mod access_fallback {
        use super::*;

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_fallback]
                fn f(&self, key: &KString) -> KValue {
                    unimplemented!()
                }
            }
        };

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_fallback]
                fn f(&self, key: &KString) -> Result<KValue> {
                    unimplemented!()
                }
            }
        };
    }

    mod access_assign_fallback {
        use super::*;

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_assign_fallback]
                fn f(&mut self, key: &KString, value: &KValue) {
                    unimplemented!()
                }
            }
        };

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_assign_fallback]
                fn f(&mut self, key: &KString, value: &KValue) -> Result<()> {
                    unimplemented!()
                }
            }
        };
    }

    mod access_override {
        use super::*;

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_override]
                fn f(&self, key: &KString) -> Option<KValue> {
                    unimplemented!()
                }
            }
        };

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_override]
                fn f(&self, key: &KString) -> Result<Option<KValue>> {
                    unimplemented!()
                }
            }
        };
    }

    mod access_assign_override {
        use super::*;

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_assign_override]
                fn f(&mut self, key: &KString, value: &KValue) -> bool {
                    unimplemented!()
                }
            }
        };

        const _: () = {
            #[derive(Clone, KotoType, KotoCopy)]
            struct Struct;

            impl KotoObject for Struct {}

            #[koto_impl]
            impl Struct {
                #[koto_access_assign_override]
                fn f(&mut self, key: &KString, value: &KValue) -> Result<bool> {
                    unimplemented!()
                }
            }
        };
    }
}

mod example {
    use super::*;

    #[derive(Clone, KotoType, KotoCopy)]
    struct Foo {
        x: f64,
    }

    impl KotoObject for Foo {}

    #[koto_impl]
    impl Foo {
        fn new(x: f64) -> Self {
            Self { x }
        }

        #[koto_access]
        fn x(&self) -> KValue {
            self.x.into()
        }

        #[koto_access_assign]
        fn set_x(&mut self, value: &KValue) -> Result<()> {
            match value {
                KValue::Number(value) => {
                    self.x = value.into();
                    Ok(())
                }
                unexpected => unexpected_type("Number", unexpected),
            }
        }

        #[koto_method(alias = "set")]
        fn reset(&mut self, args: &[KValue]) -> Result<KValue> {
            let reset_value = match args {
                [] => 0.0,
                [KValue::Number(reset_value)] => reset_value.into(),
                unexpected => return unexpected_args("||, or |Number|", unexpected),
            };
            self.x = reset_value;
            Ok(KValue::Null)
        }

        #[koto_method]
        fn add(ctx: MethodContext<Self>) -> Result<KValue> {
            match ctx.args {
                [KValue::Number(addend)] => {
                    ctx.instance_mut()?.x += f64::from(addend);
                    // Return a clone of the instance that's being modified
                    ctx.instance_result()
                }
                unexpected => unexpected_args("|Number|", unexpected),
            }
        }
    }

    #[test]
    fn test() {
        let script = r#"
v = make_foo()
assert_eq v.x, 0

v.x = 1
assert_eq v.x, 1

v.reset()
assert_eq v.x, 0

v.reset(2)
assert_eq v.x, 2

v.set()
assert_eq v.x, 0

v.set(2)
assert_eq v.x, 2

v.add(1).add(3)
assert_eq v.x, 6
"#;

        let mut koto = Koto::default();

        koto.prelude()
            .add_fn("make_foo", |_| Ok(KObject::from(Foo::new(0.0)).into()));

        koto.compile_and_run(script).unwrap();
    }
}
