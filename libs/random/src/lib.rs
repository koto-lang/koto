//! A random number module for the Koto language

use {
    koto_runtime::{num2, num4, prelude::*},
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
            unexpected => type_error_with_slice("an optional seed Number as argument", unexpected),
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
    static RNG_META: Rc<RefCell<MetaMap>> = make_rng_meta_map();

    static THREAD_RNG: RefCell<ChaChaRng> = RefCell::new(ChaChaRng(ChaCha8Rng::from_entropy()));
}

fn make_rng_meta_map() -> Rc<RefCell<MetaMap>> {
    MetaMapBuilder::<ChaChaRng>::new("Rng")
        .data_fn_mut("bool", |rng| rng.gen_bool())
        .data_fn_mut("number", |rng| rng.gen_number())
        .data_fn_mut("num2", |rng| rng.gen_num2())
        .data_fn_mut("num4", |rng| rng.gen_num4())
        .data_fn_with_args_mut("pick", |rng, args| rng.pick(args))
        .data_fn_with_args_mut("seed", |rng, args| rng.seed(args))
        .build()
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
                let index = self.0.gen_range(0..l.len());
                Ok(l.data()[index].clone())
            }
            [Map(m)] => {
                let index = self.0.gen_range(0..m.len());
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
                let index = self.0.gen_range(0..size);
                Ok(Number((start + index).into()))
            }
            [Tuple(t)] => {
                let index = self.0.gen_range(0..t.len());
                Ok(t[index].clone())
            }
            unexpected => type_error_with_slice("a List or Range as argument", unexpected),
        }
    }

    fn seed(&mut self, args: &[Value]) -> RuntimeResult {
        use Value::*;
        match args {
            [Number(n)] => {
                self.0 = ChaCha8Rng::seed_from_u64(n.to_bits());
                Ok(Null)
            }
            unexpected => type_error_with_slice("a Number as argument", unexpected),
        }
    }
}

impl ExternalData for ChaChaRng {
    fn data_type(&self) -> ValueString {
        TYPE_RNG.with(|x| x.clone())
    }
}

thread_local! {
    static TYPE_RNG: ValueString = "Rng".into();
}
