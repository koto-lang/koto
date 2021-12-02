//! A random number module for the Koto language

use {
    koto_runtime::{
        num2, num4, unexpected_type_error_with_slice, ExternalData, ExternalValue, MetaKey,
        MetaMap, Value, ValueTuple,
    },
    rand::{Rng, SeedableRng},
    rand_chacha::ChaCha20Rng,
    std::{cell::RefCell, fmt, rc::Rc},
};

pub fn make_module() -> Value {
    // The random module contains a default generator, with the default RNG interface extended with
    // the `generator` function.

    let mut module_meta = RNG_META.with(|meta| meta.borrow().clone());

    module_meta.add_fn(MetaKey::Named("generator".into()), |vm, args| {
        match vm.get_args(args) {
            // No seed, make RNG from entropy
            [] => Ok(ChaChaRng::make_external_value(ChaCha20Rng::from_entropy())),
            // RNG from seed
            [Value::Number(n)] => Ok(ChaChaRng::make_external_value(ChaCha20Rng::seed_from_u64(
                n.to_bits(),
            ))),
            unexpected => unexpected_type_error_with_slice(
                "random.generator",
                "an optional seed Number as argument",
                unexpected,
            ),
        }
    });

    let module_rng = ChaChaRng(ChaCha20Rng::from_entropy());

    Value::ExternalValue(ExternalValue::new(module_rng, module_meta))
}

thread_local!(
    static RNG_META: Rc<RefCell<MetaMap>> = {
        use Value::*;

        let mut meta = MetaMap::with_type_name("Rng");

        meta.add_named_instance_fn_mut("bool", |rng: &mut ChaChaRng, _, _| {
            Ok(Bool(rng.0.gen::<bool>()))
        });

        meta.add_named_instance_fn_mut("number", |rng: &mut ChaChaRng, _, _| {
            Ok(Number(rng.0.gen::<f64>().into()))
        });

        meta.add_named_instance_fn_mut("number2", |rng: &mut ChaChaRng, _, _| {
            let result = num2::Num2(rng.0.gen::<f64>(), rng.0.gen::<f64>());
            Ok(Num2(result))
        });

        meta.add_named_instance_fn_mut("number4", |rng: &mut ChaChaRng, _, _| {
            let result = num4::Num4(
                rng.0.gen::<f32>(),
                rng.0.gen::<f32>(),
                rng.0.gen::<f32>(),
                rng.0.gen::<f32>(),
            );
            Ok(Num4(result))
        });

        meta.add_named_instance_fn_mut("pick", |rng: &mut ChaChaRng, _, args| match args {
            [List(l)] => {
                let index = rng.0.gen_range(0, l.len());
                Ok(l.data()[index].clone())
            }
            [Map(m)] => {
                let index = rng.0.gen_range(0, m.len());
                match m.data().get_index(index) {
                    Some((key, value)) => {
                        let data = vec![key.value().clone(), value.clone()];
                        Ok(Tuple(ValueTuple::from(data)))
                    },
                    None => unreachable!(), // The index is guaranteed to be within range
                }
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
            [Tuple(t)] => {
                let index = rng.0.gen_range(0, t.data().len());
                Ok(t.data()[index].clone())
            }
            unexpected => unexpected_type_error_with_slice(
                "random.pick",
                "a List or Range as argument",
                unexpected,
            ),
        });

        meta.add_named_instance_fn_mut("seed", |rng: &mut ChaChaRng, _, args| match args {
            [Number(n)] => {
                *rng = ChaChaRng(ChaCha20Rng::seed_from_u64(n.to_bits()));
                Ok(Empty)
            }
            unexpected => unexpected_type_error_with_slice(
                "random.seed",
                "a Number as argument",
                unexpected,
            ),
        });

        Rc::new(RefCell::new(meta))
    }
);

#[derive(Debug)]
struct ChaChaRng(ChaCha20Rng);

impl ChaChaRng {
    fn make_external_value(rng: ChaCha20Rng) -> Value {
        let result =
            ExternalValue::with_shared_meta_map(ChaChaRng(rng), RNG_META.with(|meta| meta.clone()));

        Value::ExternalValue(result)
    }
}

impl ExternalData for ChaChaRng {
    fn value_type(&self) -> String {
        "Rng".to_string()
    }
}

impl fmt::Display for ChaChaRng {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rng")
    }
}
