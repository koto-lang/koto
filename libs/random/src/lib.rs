//! A random number module for the Koto language

use koto_runtime::{derive::*, prelude::*, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::cell::RefCell;

pub fn make_module() -> KMap {
    let result = KMap::with_type("random");

    result.add_fn("bool", |ctx| match ctx.args() {
        [] => THREAD_RNG.with_borrow_mut(|rng| rng.bool()),
        unexpected => unexpected_args("||", unexpected),
    });

    result.add_fn("generator", |ctx| {
        let rng = match ctx.args() {
            // No seed, make RNG from entropy
            [] => ChaCha8Rng::from_entropy(),
            // RNG from seed
            [KValue::Number(n)] => ChaCha8Rng::seed_from_u64(n.to_bits()),
            unexpected => return unexpected_args("||, or |Number|", unexpected),
        };

        Ok(ChaChaRng::make_value(rng))
    });

    result.add_fn("number", |ctx| match ctx.args() {
        [] => THREAD_RNG.with_borrow_mut(|rng| rng.number()),
        unexpected => unexpected_args("||", unexpected),
    });

    result.add_fn("pick", |ctx| {
        THREAD_RNG.with_borrow_mut(|rng| match ctx.args() {
            [arg] => rng.pick_inner(arg.clone(), ctx.vm),
            unexpected => unexpected_args("|Indexable|", unexpected),
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
            unexpected => unexpected_args("|Indexable|", unexpected),
        }
    }

    fn pick_inner(&mut self, arg: KValue, vm: &mut KotoVm) -> Result<KValue> {
        use KValue::*;

        match arg {
            List(l) => {
                if !l.is_empty() {
                    let index = self.0.gen_range(0..l.len());
                    Ok(l.data()[index].clone())
                } else {
                    Ok(Null)
                }
            }
            Tuple(t) => {
                if !t.is_empty() {
                    let index = self.0.gen_range(0..t.len());
                    Ok(t[index].clone())
                } else {
                    Ok(Null)
                }
            }
            Range(r) => {
                let full_range = r.as_sorted_range();
                if !full_range.is_empty() {
                    let result = self.0.gen_range(full_range);
                    Ok(result.into())
                } else {
                    Ok(Null)
                }
            }
            Map(m) if !m.contains_meta_key(&BinaryOp::Index.into()) => {
                if !m.is_empty() {
                    let index = self.0.gen_range(0..m.len());
                    match m.data().get_index(index) {
                        Some((key, value)) => {
                            Ok(Tuple(KTuple::from(&[key.value().clone(), value.clone()])))
                        }
                        None => unreachable!(), // The index is guaranteed to be within range
                    }
                } else {
                    Ok(Null)
                }
            }
            // Cover other cases like objects and maps with @[] ops via the vm
            input => match vm.run_unary_op(UnaryOp::Size, input.clone())? {
                Number(size) => {
                    if size > 0 {
                        let index = self.0.gen_range(0..(size.as_i64() as usize));
                        vm.run_binary_op(BinaryOp::Index, input.clone(), index.into())
                    } else {
                        Ok(Null)
                    }
                }
                unexpected => unexpected_type("a Number from @size", &unexpected),
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
            unexpected => unexpected_args("|Number|", unexpected),
        }
    }
}

impl KotoObject for ChaChaRng {}

thread_local! {
    static THREAD_RNG: RefCell<ChaChaRng> = RefCell::new(ChaChaRng(ChaCha8Rng::from_entropy()));
}
