//! A random number module for the Koto language

use koto_runtime::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::cell::RefCell;

pub fn make_module() -> ValueMap {
    let result = ValueMap::new();

    result.add_fn("bool", |_, _| {
        THREAD_RNG.with(|rng| rng.borrow_mut().gen_bool())
    });

    result.add_fn("generator", |vm, args| {
        let rng = match vm.get_args(args) {
            // No seed, make RNG from entropy
            [] => ChaCha8Rng::from_entropy(),
            // RNG from seed
            [Value::Number(n)] => ChaCha8Rng::seed_from_u64(n.to_bits()),
            unexpected => {
                return type_error_with_slice("an optional seed Number as argument", unexpected)
            }
        };

        Ok(ChaChaRng::make_value(rng))
    });

    result.add_fn("number", |_, _| {
        THREAD_RNG.with(|rng| rng.borrow_mut().gen_number())
    });

    result.add_fn("pick", |vm, args| {
        THREAD_RNG.with(|rng| rng.borrow_mut().pick(vm.get_args(args)))
    });

    result.add_fn("seed", |vm, args| {
        THREAD_RNG.with(|rng| rng.borrow_mut().seed(vm.get_args(args)))
    });

    result
}

#[derive(Clone, Debug)]
struct ChaChaRng(ChaCha8Rng);

impl ChaChaRng {
    fn make_value(rng: ChaCha8Rng) -> Value {
        Object::from(Self(rng)).into()
    }

    fn gen_bool(&mut self) -> RuntimeResult {
        Ok(self.0.gen::<bool>().into())
    }

    fn gen_number(&mut self) -> RuntimeResult {
        Ok(self.0.gen::<f64>().into())
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
                let result = self.0.gen_range(r.as_sorted_range());
                Ok(result.into())
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

impl KotoType for ChaChaRng {
    const TYPE: &'static str = "Rng";
}

impl KotoObject for ChaChaRng {
    fn object_type(&self) -> ValueString {
        RNG_TYPE_STRING.with(|s| s.clone())
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        RNG_ENTRIES.with(|entries| entries.get(key).cloned())
    }
}

fn rng_entries() -> DataMap {
    ObjectEntryBuilder::<ChaChaRng>::new()
        .method("bool", |ctx| ctx.instance_mut()?.gen_bool())
        .method("number", |ctx| ctx.instance_mut()?.gen_number())
        .method("pick", |ctx| ctx.instance_mut()?.pick(ctx.args))
        .method("seed", |ctx| ctx.instance_mut()?.seed(ctx.args))
        .build()
}

thread_local! {
    static THREAD_RNG: RefCell<ChaChaRng> = RefCell::new(ChaChaRng(ChaCha8Rng::from_entropy()));
    static RNG_TYPE_STRING: ValueString = ChaChaRng::TYPE.into();
    static RNG_ENTRIES: DataMap = rng_entries();
}
