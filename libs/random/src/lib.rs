//! A random number module for the Koto language

use koto_runtime::{derive::*, prelude::*, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::cell::RefCell;

pub fn make_module() -> KMap {
    let result = KMap::with_type("random");

    result.add_fn("bool", |_| THREAD_RNG.with_borrow_mut(|rng| rng.bool()));

    result.add_fn("generator", |ctx| {
        let rng = match ctx.args() {
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

    result.add_fn("number", |_| THREAD_RNG.with_borrow_mut(|rng| rng.number()));

    result.add_fn("pick", |ctx| {
        THREAD_RNG.with_borrow_mut(|rng| rng.pick(ctx.args()))
    });

    result.add_fn("seed", |ctx| {
        THREAD_RNG.with_borrow_mut(|rng| rng.seed(ctx.args()))
    });

    result
}

#[derive(Clone, Debug, KotoCopy, KotoType)]
#[koto(type_name = "Rng")]
struct ChaChaRng(ChaCha8Rng);

#[koto_impl(runtime = koto_runtime)]
impl ChaChaRng {
    fn make_value(rng: ChaCha8Rng) -> Value {
        KObject::from(Self(rng)).into()
    }

    #[koto_method]
    fn bool(&mut self) -> Result<Value> {
        Ok(self.0.gen::<bool>().into())
    }

    #[koto_method]
    fn number(&mut self) -> Result<Value> {
        Ok(self.0.gen::<f64>().into())
    }

    #[koto_method]
    fn pick(&mut self, args: &[Value]) -> Result<Value> {
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
                        Ok(Tuple(KTuple::from(data)))
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
            unexpected => type_error_with_slice("a container or range as argument", unexpected),
        }
    }

    #[koto_method]
    fn seed(&mut self, args: &[Value]) -> Result<Value> {
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

impl KotoObject for ChaChaRng {}

thread_local! {
    static THREAD_RNG: RefCell<ChaChaRng> = RefCell::new(ChaChaRng(ChaCha8Rng::from_entropy()));
}
