//! A random number module for the Koto language

use {
    koto_runtime::{
        get_external_instance, num2, num4, runtime_error, ExternalValue, Value, ValueMap,
    },
    rand::{Rng, SeedableRng},
    rand_chacha::ChaCha20Rng,
    std::fmt,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    // The random module contains a default generator
    let mut result = ChaChaRng::make_value_map();
    // random.generator is available to create custom generators
    result.add_fn("generator", {
        let vtable = result.clone();
        move |vm, args| {
            match vm.get_args(args) {
                [] => Ok(Value::make_external_value(ChaChaRng(ChaCha20Rng::from_entropy()), vtable.clone())),
                [Number(n)] => Ok(Value::make_external_value(
                        ChaChaRng(ChaCha20Rng::seed_from_u64(n.to_bits())),
                        vtable.clone()
                    )),
                _ => runtime_error!("random.generator - expected no arguments, or seed number"),
            }
        }
    });
    result
}

#[derive(Debug)]
struct ChaChaRng(ChaCha20Rng);

impl ChaChaRng {
    fn make_value_map() -> ValueMap {
        use Value::*;

        let mut vtable = ValueMap::new();

        vtable.add_instance_fn("bool", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "bool", Self, rng, {
                Ok(Bool(rng.0.gen::<bool>()))
            })
        });

        vtable.add_instance_fn("number", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "number", Self, rng, {
                Ok(Number(rng.0.gen::<f64>().into()))
            })
        });

        vtable.add_instance_fn("number2", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "number2", Self, rng, {
                let vtable = num2::Num2(rng.0.gen::<f64>(), rng.0.gen::<f64>());
                Ok(Num2(vtable))
            })
        });

        vtable.add_instance_fn("number4", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "number4", Self, rng, {
                let vtable = num4::Num4(
                    rng.0.gen::<f32>(),
                    rng.0.gen::<f32>(),
                    rng.0.gen::<f32>(),
                    rng.0.gen::<f32>(),
                );
                Ok(Num4(vtable))
            })
        });

        vtable.add_instance_fn("pick", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "number", Self, rng, {
                match &args[1..] {
                    [List(l)] => {
                        let index = rng.0.gen_range(0, l.len());
                        Ok(l.data()[index].clone())
                    }
                    [Range(r)] => {
                        let (start, end) = if r.end > r.start {
                            (r.start, r.end)
                        } else {
                            (r.end, r.start)
                        };
                        let size = end - start;
                        let index = rng.0.gen_range(0, size);
                        Ok(Number((start + index).into()))
                    }
                    _ => runtime_error!("random.pick - expected list or range as argument"),
                }
            })
        });

        vtable.add_instance_fn("seed", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "seed", Self, rng, {
                match &args[1..] {
                    [Number(n)] => {
                        *rng = ChaChaRng(ChaCha20Rng::seed_from_u64(n.to_bits()));
                        Ok(Empty)
                    }
                    _ => runtime_error!("random.seed - expected number as argument"),
                }
            })
        });

        vtable
    }
}

impl ExternalValue for ChaChaRng {
    fn value_type(&self) -> String {
        "Rng".to_string()
    }
}

impl fmt::Display for ChaChaRng {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rng")
    }
}
