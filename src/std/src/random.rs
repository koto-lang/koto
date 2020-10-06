use {
    crate::{get_external_instance, ExternalValue},
    koto_parser::{num2, num4},
    koto_runtime::{external_error, make_external_value, Value, ValueMap},
    rand::{Rng, SeedableRng},
    rand_chacha::ChaCha20Rng,
    std::fmt,
};

pub fn register(prelude: &mut ValueMap) {
    use Value::*;

    let mut random = ChaChaRng::make_value_map(ChaCha20Rng::from_entropy());

    random.add_fn("generator", |vm, args| match vm.get_args(args) {
        [] => Ok(Map(ChaChaRng::make_value_map(ChaCha20Rng::from_entropy()))),
        [Number(n)] => Ok(Map(ChaChaRng::make_value_map(ChaCha20Rng::seed_from_u64(
            n.to_bits(),
        )))),
        _ => external_error!("random.generator - expected no arguments, or seed number"),
    });

    prelude.add_map("random", random);
}

#[derive(Debug)]
struct ChaChaRng(ChaCha20Rng);

impl ChaChaRng {
    fn make_value_map(rng: ChaCha20Rng) -> ValueMap {
        use Value::*;

        let mut result = ValueMap::new();

        result.add_instance_fn("bool", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "bool", Self, rng, {
                Ok(Bool(rng.0.gen::<bool>()))
            })
        });

        result.add_instance_fn("number", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "number", Self, rng, {
                Ok(Number(rng.0.gen::<f64>()))
            })
        });

        result.add_instance_fn("number2", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "number2", Self, rng, {
                let result = num2::Num2(rng.0.gen::<f64>(), rng.0.gen::<f64>());
                Ok(Num2(result))
            })
        });

        result.add_instance_fn("number4", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "number4", Self, rng, {
                let result = num4::Num4(
                    rng.0.gen::<f32>(),
                    rng.0.gen::<f32>(),
                    rng.0.gen::<f32>(),
                    rng.0.gen::<f32>(),
                );
                Ok(Num4(result))
            })
        });

        result.add_instance_fn("pick", |vm, args| {
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
                        Ok(Number((start + index) as f64))
                    }
                    _ => external_error!("random.pick - expected list or range as argument"),
                }
            })
        });

        result.add_instance_fn("seed", |vm, args| {
            let args = vm.get_args(args);
            get_external_instance!(args, "random", "seed", Self, rng, {
                match &args[1..] {
                    [Number(n)] => {
                        *rng = ChaChaRng(ChaCha20Rng::seed_from_u64(n.to_bits()));
                        Ok(Empty)
                    }
                    _ => external_error!("random.seed - expected number as argument"),
                }
            })
        });

        result.insert(Value::ExternalDataId, make_external_value(Self(rng)));
        result
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
