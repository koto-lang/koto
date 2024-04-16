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
            [KValue::Number(n)] => ChaCha8Rng::seed_from_u64(n.to_bits()),
            unexpected => {
                return type_error_with_slice("an optional seed Number as argument", unexpected)
            }
        };

        Ok(ChaChaRng::make_value(rng))
    });

    result.add_fn("number", |_| THREAD_RNG.with_borrow_mut(|rng| rng.number()));

    result.add_fn("pick", |ctx| {
        THREAD_RNG.with_borrow_mut(|rng| match ctx.args() {
            [arg] => rng.pick_inner(arg.clone(), ctx.vm),
            unexpected => type_error_with_slice("a single argument", unexpected),
        })
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
    fn make_value(rng: ChaCha8Rng) -> KValue {
        KObject::from(Self(rng)).into()
    }

    #[koto_method]
    fn bool(&mut self) -> Result<KValue> {
        Ok(self.0.gen::<bool>().into())
    }

    #[koto_method]
    fn number(&mut self) -> Result<KValue> {
        Ok(self.0.gen::<f64>().into())
    }

    #[koto_method]
    fn pick(ctx: MethodContext<Self>) -> Result<KValue> {
        match ctx.args {
            [arg] => ctx
                .instance_mut()?
                .pick_inner(arg.clone(), &mut ctx.vm.spawn_shared_vm()),
            unexpected => type_error_with_slice("a single argument", unexpected),
        }
    }

    fn pick_inner(&mut self, arg: KValue, vm: &mut KotoVm) -> Result<KValue> {
        use KValue::*;

        match arg {
            // Handle basic containers directly
            List(l) => {
                let index = self.0.gen_range(0..l.len());
                Ok(l.data()[index].clone())
            }
            Range(r) => {
                let result = self.0.gen_range(r.as_sorted_range());
                Ok(result.into())
            }
            Tuple(t) => {
                let index = self.0.gen_range(0..t.len());
                Ok(t[index].clone())
            }
            Map(m) if !m.contains_meta_key(&BinaryOp::Index.into()) => {
                let index = self.0.gen_range(0..m.len());
                match m.data().get_index(index) {
                    Some((key, value)) => {
                        Ok(Tuple(KTuple::from(&[key.value().clone(), value.clone()])))
                    }
                    None => unreachable!(), // The index is guaranteed to be within range
                }
            }
            // Cover other cases like objects and maps with @[] ops via the vm
            input => match vm.run_unary_op(UnaryOp::Size, input.clone())? {
                Number(size) => {
                    let index = self.0.gen_range(0..(size.as_i64() as usize));
                    vm.run_binary_op(BinaryOp::Index, input.clone(), index.into())
                }
                unexpected => type_error("a number from @size", &unexpected),
            },
        }
    }

    #[koto_method]
    fn seed(&mut self, args: &[KValue]) -> Result<KValue> {
        use KValue::*;
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
