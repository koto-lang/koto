//! A random number module for the Koto language

use {
    koto_runtime::{
        num2, num4, unexpected_type_error_with_slice, ExternalData, ExternalValue, MetaMap,
        RuntimeResult, Value, ValueMap, ValueTuple,
    },
    rand::{Rng, SeedableRng},
    rand_chacha::ChaCha8Rng,
    std::{cell::RefCell, rc::Rc},
};

pub fn make_module() -> ValueMap {
    let result = ValueMap::new();

    result.add_fn("bool", |_, _| {
        THREAD_RNG.with(|rng| rng.borrow_mut().gen_bool())
    });

    result.add_fn("generator", |vm, args| {
        match vm.get_args(args) {
            // No seed, make RNG from entropy
            [] => Ok(ChaChaRng::make_external_value(ChaCha8Rng::from_entropy())),
            // RNG from seed
            [Value::Number(n)] => Ok(ChaChaRng::make_external_value(ChaCha8Rng::seed_from_u64(
                n.to_bits(),
            ))),
            unexpected => unexpected_type_error_with_slice(
                "random.generator",
                "an optional seed Number as argument",
                unexpected,
            ),
        }
    });

    result.add_fn("number", |_, _| {
        THREAD_RNG.with(|rng| rng.borrow_mut().gen_number())
    });

    result.add_fn("num2", |_, _| {
        THREAD_RNG.with(|rng| rng.borrow_mut().gen_num2())
    });

    result.add_fn("num4", |_, _| {
        THREAD_RNG.with(|rng| rng.borrow_mut().gen_num4())
    });

    result.add_fn("pick", |vm, args| {
        THREAD_RNG.with(|rng| rng.borrow_mut().pick(vm.get_args(args)))
    });

    result.add_fn("seed", |vm, args| {
        THREAD_RNG.with(|rng| rng.borrow_mut().seed(vm.get_args(args)))
    });

    result
}

thread_local! {
    static RNG_META: Rc<RefCell<MetaMap>> = {
        let mut meta = MetaMap::with_type_name("Rng");

        meta.add_named_instance_fn_mut("bool", |rng: &mut ChaChaRng, _, _| rng.gen_bool());
        meta.add_named_instance_fn_mut("number", |rng: &mut ChaChaRng, _, _| rng.gen_number());
        meta.add_named_instance_fn_mut("num2", |rng: &mut ChaChaRng, _, _| rng.gen_num2());
        meta.add_named_instance_fn_mut("num4", |rng: &mut ChaChaRng, _, _| rng.gen_num4());
        meta.add_named_instance_fn_mut("pick", |rng: &mut ChaChaRng, _, args| rng.pick(args));
        meta.add_named_instance_fn_mut("seed", |rng: &mut ChaChaRng, _, args| rng.seed(args));

        Rc::new(RefCell::new(meta))
    };

    static THREAD_RNG: RefCell<ChaChaRng> = RefCell::new(ChaChaRng(ChaCha8Rng::from_entropy()));
}

#[derive(Debug)]
struct ChaChaRng(ChaCha8Rng);

impl ChaChaRng {
    fn make_external_value(rng: ChaCha8Rng) -> Value {
        let result =
            ExternalValue::with_shared_meta_map(ChaChaRng(rng), RNG_META.with(|meta| meta.clone()));

        Value::ExternalValue(result)
    }

    fn gen_bool(&mut self) -> RuntimeResult {
        Ok(self.0.gen::<bool>().into())
    }

    fn gen_number(&mut self) -> RuntimeResult {
        Ok(self.0.gen::<f64>().into())
    }

    fn gen_num2(&mut self) -> RuntimeResult {
        let result = num2::Num2(self.0.gen::<f64>(), self.0.gen::<f64>());
        Ok(Value::Num2(result))
    }

    fn gen_num4(&mut self) -> RuntimeResult {
        let result = num4::Num4(
            self.0.gen::<f32>(),
            self.0.gen::<f32>(),
            self.0.gen::<f32>(),
            self.0.gen::<f32>(),
        );
        Ok(Value::Num4(result))
    }

    fn pick(&mut self, args: &[Value]) -> RuntimeResult {
        use Value::*;

        match args {
            [List(l)] => {
                let index = self.0.gen_range(0, l.len());
                Ok(l.data()[index].clone())
            }
            [Map(m)] => {
                let index = self.0.gen_range(0, m.len());
                match m.data().get_index(index) {
                    Some((key, value)) => {
                        let data = vec![key.value().clone(), value.clone()];
                        Ok(Tuple(ValueTuple::from(data)))
                    }
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
                let index = self.0.gen_range(0, size);
                Ok(Number((start + index).into()))
            }
            [Tuple(t)] => {
                let index = self.0.gen_range(0, t.data().len());
                Ok(t.data()[index].clone())
            }
            unexpected => unexpected_type_error_with_slice(
                "random.pick",
                "a List or Range as argument",
                unexpected,
            ),
        }
    }

    fn seed(&mut self, args: &[Value]) -> RuntimeResult {
        use Value::*;
        match args {
            [Number(n)] => {
                self.0 = ChaCha8Rng::seed_from_u64(n.to_bits());
                Ok(Empty)
            }
            unexpected => {
                unexpected_type_error_with_slice("random.seed", "a Number as argument", unexpected)
            }
        }
    }
}

impl ExternalData for ChaChaRng {
    fn value_type(&self) -> String {
        "Rng".to_string()
    }
}
