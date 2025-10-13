//! A random number module for the Koto language

use koto_runtime::{Result, derive::*, prelude::*};
use rand::{Rng, SeedableRng, seq::SliceRandom};
use rand_xoshiro::Xoshiro256PlusPlus;
use std::cell::RefCell;

pub fn make_module() -> KMap {
    koto_fn! {
        runtime = koto_runtime;

        fn gen_bool() -> Result<KValue> {
            THREAD_RNG.with_borrow_mut(|rng| rng.bool())
        }

        fn generator() -> KValue {
            // No seed, use a randomly seeded rng
            Xoshiro256PlusPlusRng::make_value(Xoshiro256PlusPlus::from_os_rng())
        }

        fn generator(seed: KNumber) -> KValue {
            Xoshiro256PlusPlusRng::make_value(Xoshiro256PlusPlus::seed_from_u64(seed.to_bits()))
        }

        fn gen_number() -> Result<KValue> {
            THREAD_RNG.with_borrow_mut(|rng| rng.number())
        }

        fn pick(arg: KValue, vm: &mut KotoVm) -> Result<KValue> {
            THREAD_RNG.with_borrow_mut(|rng| rng.pick_inner(arg, vm))
        }

        fn seed(n: &KNumber) {
            THREAD_RNG.with_borrow_mut(|rng| rng.seed_inner(n));
        }

        fn shuffle(arg: KValue, vm: &mut KotoVm) -> Result<KValue> {
            THREAD_RNG.with_borrow_mut(|rng| rng.shuffle_inner(arg, vm))
        }
    }

    let result = KMap::with_type("random");

    result.add_fn("bool", gen_bool);
    result.add_fn("generator", generator);
    result.add_fn("number", gen_number);
    result.add_fn("pick", pick);
    result.add_fn("seed", seed);
    result.add_fn("shuffle", shuffle);

    result
}

#[derive(Clone, Debug, KotoCopy, KotoType)]
#[koto(runtime = koto_runtime, type_name = "Rng")]
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
            Map(m) if !m.contains_meta_key(&ReadOp::Index.into()) => {
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
                        vm.run_read_op(ReadOp::Index, input.clone(), index.into())
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
                self.seed_inner(n);
                Ok(Null)
            }
            unexpected => unexpected_args("|Number|", unexpected),
        }
    }

    fn seed_inner(&mut self, n: &KNumber) {
        self.0 = Xoshiro256PlusPlus::seed_from_u64(n.to_bits());
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
            Map(m) if m.contains_meta_key(&WriteOp::IndexMut.into()) => {
                let index_mut = m.get_meta_value(&WriteOp::IndexMut.into()).unwrap();

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
                            let value_i = vm.run_read_op(ReadOp::Index, arg.clone(), i.into())?;
                            let value_j = vm.run_read_op(ReadOp::Index, arg.clone(), j.into())?;
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
            Map(m) => {
                let mut data = m.data_mut();
                for i in (1..data.len()).rev() {
                    let j = self.0.random_range(0..(i + 1));
                    data.swap_indices(i, j);
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
