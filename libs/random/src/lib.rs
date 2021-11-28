//! A random number module for the Koto language

use {
    koto_runtime::{
        num2, num4, runtime_error, ExternalData, ExternalValue, MetaKey, MetaMap, Value,
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
            _ => runtime_error!("random.generator - expected no arguments, or seed number"),
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
        });

        meta.add_named_instance_fn_mut("seed", |rng: &mut ChaChaRng, _, args| match args {
            [Number(n)] => {
                *rng = ChaChaRng(ChaCha20Rng::seed_from_u64(n.to_bits()));
                Ok(Empty)
            }
            _ => runtime_error!("random.seed - expected number as argument"),
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
