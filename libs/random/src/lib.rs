//! A random number module for the Koto language

use koto_runtime::{Result, derive::*, prelude::*};
use rand::{Rng, SeedableRng, seq::SliceRandom};
use rand_xoshiro::Xoshiro256PlusPlus;
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
            [] => Xoshiro256PlusPlus::from_os_rng(),
            // RNG from seed
            [KValue::Number(n)] => Xoshiro256PlusPlus::seed_from_u64(n.to_bits()),
            unexpected => return unexpected_args("||, or |Number|", unexpected),
        };

        Ok(Xoshiro256PlusPlusRng::make_value(rng))
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

    result.add_fn("shuffle", |ctx| {
        THREAD_RNG.with_borrow_mut(|rng| match ctx.args() {
            [arg] => rng.shuffle_inner(arg.clone(), ctx.vm),
            unexpected => unexpected_args("|Indexable|", unexpected),
        })
    });

    result
}

#[derive(Clone, Debug, KotoCopy, KotoType)]
#[koto(type_name = "Rng")]
struct Xoshiro256PlusPlusRng(Xoshiro256PlusPlus);

#[koto_impl(runtime = koto_runtime)]
impl Xoshiro256PlusPlusRng {
    fn make_value(rng: Xoshiro256PlusPlus) -> KValue {
        KObject::from(Self(rng)).into()
    }

    #[koto_method]
    fn bool(&mut self) -> Result<KValue> {
        Ok(self.0.random::<bool>().into())
    }

    #[koto_method]
    fn number(&mut self) -> Result<KValue> {
        Ok(self.0.random::<f64>().into())
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
                    let index = self.0.random_range(0..l.len());
                    Ok(l.data()[index].clone())
                } else {
                    Ok(Null)
                }
            }
            Tuple(t) => {
                if !t.is_empty() {
                    let index = self.0.random_range(0..t.len());
                    Ok(t[index].clone())
                } else {
                    Ok(Null)
                }
            }
            Range(r) => {
                let full_range = r.as_sorted_range();
                if !full_range.is_empty() {
                    let result = self.0.random_range(full_range);
                    Ok(result.into())
                } else {
                    Ok(Null)
                }
            }
            Map(m) if !m.contains_meta_key(&BinaryOp::Index.into()) => {
                if !m.is_empty() {
                    let index = self.0.random_range(0..m.len());
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
            // Cover other cases like objects and maps with @size/@index ops
            input => match vm.run_unary_op(UnaryOp::Size, input.clone())? {
                Number(size) => {
                    if size > 0 {
                        let index = self.0.random_range(0..usize::from(size));
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
                self.0 = Xoshiro256PlusPlus::seed_from_u64(n.to_bits());
                Ok(Null)
            }
            unexpected => unexpected_args("|Number|", unexpected),
        }
    }

    #[koto_method]
    fn shuffle(ctx: MethodContext<Self>) -> Result<KValue> {
        match ctx.args {
            [arg] => ctx
                .instance_mut()?
                .shuffle_inner(arg.clone(), &mut ctx.vm.spawn_shared_vm()),
            unexpected => unexpected_args("|Indexable|", unexpected),
        }
    }

    fn shuffle_inner(&mut self, arg: KValue, vm: &mut KotoVm) -> Result<KValue> {
        use KValue::*;

        match &arg {
            List(l) => {
                l.data_mut().shuffle(&mut self.0);
            }
            Map(m) if !m.contains_meta_key(&MetaKey::IndexMut) => {
                let mut data = m.data_mut();
                for i in (1..data.len()).rev() {
                    let j = self.0.random_range(0..(i + 1));
                    data.swap_indices(i, j);
                }
            }
            Map(m) if m.contains_meta_key(&MetaKey::IndexMut) => {
                let index_mut = m.get_meta_value(&MetaKey::IndexMut).unwrap();

                match vm.run_unary_op(UnaryOp::Size, arg.clone())? {
                    Number(size) => {
                        if size <= 0 {
                            return runtime_error!("expected a positive @size, found {}", size);
                        }

                        for i in (1..usize::from(size)).rev() {
                            let j = self.0.random_range(0..(i + 1));
                            if i == j {
                                continue;
                            }
                            let value_i =
                                vm.run_binary_op(BinaryOp::Index, arg.clone(), i.into())?;
                            let value_j =
                                vm.run_binary_op(BinaryOp::Index, arg.clone(), j.into())?;
                            vm.call_instance_function(
                                arg.clone(),
                                index_mut.clone(),
                                &[i.into(), value_j],
                            )?;
                            vm.call_instance_function(
                                arg.clone(),
                                index_mut.clone(),
                                &[j.into(), value_i],
                            )?;
                        }
                    }
                    unexpected => return unexpected_type("a Number from @size", &unexpected),
                }
            }
            Object(o) => {
                let mut o_borrow = o.try_borrow_mut()?;
                let Some(size) = o_borrow.size() else {
                    return runtime_error!("{} has an unknown size", o_borrow.type_string());
                };

                for i in (1..size).rev() {
                    let j = self.0.random_range(0..(i + 1));
                    if i == j {
                        continue;
                    }
                    let i = KValue::from(i);
                    let j = KValue::from(j);
                    let value_i = o_borrow.index(&i)?;
                    let value_j = o_borrow.index(&j)?;
                    o_borrow.index_mut(&i, &value_j)?;
                    o_borrow.index_mut(&j, &value_i)?;
                }
            }
            unexpected => return unexpected_type("|Indexable|", unexpected),
        }

        Ok(arg)
    }
}

impl KotoObject for Xoshiro256PlusPlusRng {}

thread_local! {
    static THREAD_RNG: RefCell<Xoshiro256PlusPlusRng>
        = RefCell::new(Xoshiro256PlusPlusRng(Xoshiro256PlusPlus::from_os_rng()));
}
