//! A random number module for the Koto language

use koto_runtime::{Result, derive::*, prelude::*};
use rand::{Rng, SeedableRng, seq::SliceRandom};
use rand_xoshiro::Xoshiro256PlusPlus;
use std::cell::RefCell;

pub fn make_module() -> KMap {
    koto_fn! {
        runtime = koto_runtime;

        fn gen_bool() -> bool {
            THREAD_RNG.with_borrow_mut(|rng| rng.bool())
        }

        fn generator() -> KValue {
            // No seed, use a randomly seeded rng
            Xoshiro256PlusPlusRng::make_value(Xoshiro256PlusPlus::from_os_rng())
        }

        fn generator(seed: KNumber) -> KValue {
            Xoshiro256PlusPlusRng::make_value(Xoshiro256PlusPlus::seed_from_u64(seed.to_bits()))
        }

        fn gen_number() -> f64 {
            THREAD_RNG.with_borrow_mut(|rng| rng.number())
        }

        fn seed(n: &KNumber) {
            THREAD_RNG.with_borrow_mut(|rng| rng.seed_inner(n));
        }
    }

    let result = KMap::with_type("random");

    result.add_fn("bool", gen_bool);
    result.add_fn("generator", generator);
    result.add_fn("number", gen_number);
    result.add_vm_fn("pick", |ctx| match ctx.args() {
        [arg] => pick_inner(RngSource::Thread, arg.clone(), ctx),
        unexpected => unexpected_args("|Indexable|", unexpected).map(FunctionOutput::Ready),
    });
    result.add_fn("seed", seed);
    result.add_vm_fn("shuffle", |ctx| match ctx.args() {
        [arg] => shuffle_inner(RngSource::Thread, arg.clone(), ctx),
        unexpected => unexpected_args("|Indexable|", unexpected).map(FunctionOutput::Ready),
    });

    result
}

#[derive(Clone, Debug, KotoCopy, KotoType)]
#[koto(runtime = koto_runtime, type_name = "Rng")]
struct Xoshiro256PlusPlusRng(Xoshiro256PlusPlus);

#[derive(Clone)]
enum RngSource {
    Thread,
    Object(KObject),
}

impl RngSource {
    fn with_rng<T>(&self, f: impl FnOnce(&mut Xoshiro256PlusPlus) -> Result<T>) -> Result<T> {
        match self {
            Self::Thread => THREAD_RNG.with_borrow_mut(|rng| f(&mut rng.0)),
            Self::Object(object) => {
                let mut rng = object.cast_mut::<Xoshiro256PlusPlusRng>()?;
                f(&mut rng.0)
            }
        }
    }

    fn random_index(&self, end: usize) -> Result<usize> {
        self.with_rng(|rng| Ok(rng.random_range(0..end)))
    }
}

#[koto_impl(runtime = koto_runtime)]
impl Xoshiro256PlusPlusRng {
    fn make_value(rng: Xoshiro256PlusPlus) -> KValue {
        KObject::from(Self(rng)).into()
    }

    #[koto_method]
    fn bool(&mut self) -> bool {
        self.0.random()
    }

    #[koto_method]
    fn number(&mut self) -> f64 {
        self.0.random()
    }

    #[koto_vm_method]
    fn pick(ctx: &mut VmCallContext) -> Result<FunctionOutput> {
        match rng_instance_and_args(ctx)? {
            (rng, [arg]) => pick_inner(rng, arg.clone(), ctx),
            (_, unexpected) => {
                unexpected_args("|Indexable|", unexpected).map(FunctionOutput::Ready)
            }
        }
    }

    #[koto_method]
    fn seed(&mut self, n: &KNumber) {
        self.seed_inner(n);
    }

    fn seed_inner(&mut self, n: &KNumber) {
        self.0 = Xoshiro256PlusPlus::seed_from_u64(n.to_bits());
    }

    #[koto_vm_method]
    fn shuffle(ctx: &mut VmCallContext) -> Result<FunctionOutput> {
        match rng_instance_and_args(ctx)? {
            (rng, [arg]) => shuffle_inner(rng, arg.clone(), ctx),
            (_, unexpected) => {
                unexpected_args("|Indexable|", unexpected).map(FunctionOutput::Ready)
            }
        }
    }
}

fn rng_instance_and_args<'a>(ctx: &'a VmCallContext<'_>) -> Result<(RngSource, &'a [KValue])> {
    let (instance, args) = ctx.instance_and_args(
        |i| matches!(i, KValue::Object(_)),
        <Xoshiro256PlusPlusRng as KotoType>::type_static(),
    )?;

    match instance {
        KValue::Object(object) => {
            object.cast::<Xoshiro256PlusPlusRng>()?;
            Ok((RngSource::Object(object.clone()), args))
        }
        _ => unreachable!(),
    }
}

impl KotoObject for Xoshiro256PlusPlusRng {}

fn pick_inner(rng: RngSource, arg: KValue, ctx: &mut VmCallContext) -> Result<FunctionOutput> {
    use KValue::*;

    match arg {
        List(l) => {
            if !l.is_empty() {
                let index = rng.random_index(l.len())?;
                Ok(FunctionOutput::Ready(l.data()[index].clone()))
            } else {
                Ok(FunctionOutput::Ready(Null))
            }
        }
        Tuple(t) => {
            if !t.is_empty() {
                let index = rng.random_index(t.len())?;
                Ok(FunctionOutput::Ready(t[index].clone()))
            } else {
                Ok(FunctionOutput::Ready(Null))
            }
        }
        Range(r) => {
            let full_range = r.as_bounded_range();
            if !full_range.is_empty() {
                let result = rng.with_rng(|rng| Ok(rng.random_range(full_range)))?;
                Ok(FunctionOutput::Ready(result.into()))
            } else {
                Ok(FunctionOutput::Ready(Null))
            }
        }
        Map(m) if !m.contains_meta_key(&ReadOp::Index.into()) => {
            if !m.is_empty() {
                let index = rng.random_index(m.len())?;
                match m.data().get_index(index) {
                    Some((key, value)) => Ok(FunctionOutput::Ready(Tuple(KTuple::from(&[
                        key.value().clone(),
                        value.clone(),
                    ])))),
                    None => unreachable!(), // The index is guaranteed to be within range
                }
            } else {
                Ok(FunctionOutput::Ready(Null))
            }
        }
        // Cover other cases like objects and maps with @size/@index ops.
        input => ctx.run_with_vm(move |mut vm| async move {
            match vm.run_unary_op(UnaryOp::Size, input.clone()).await? {
                Number(size) => {
                    if size > 0 {
                        let index = rng.random_index(usize::from(size))?;
                        vm.run_read_op(ReadOp::Index, input.clone(), index.into())
                            .await
                    } else {
                        Ok(Null)
                    }
                }
                unexpected => unexpected_type("a Number from @size", &unexpected),
            }
        }),
    }
}

fn shuffle_inner(rng: RngSource, arg: KValue, ctx: &mut VmCallContext) -> Result<FunctionOutput> {
    use KValue::*;

    match arg {
        List(l) => {
            rng.with_rng(|rng| {
                l.data_mut().shuffle(rng);
                Ok(())
            })?;
            Ok(FunctionOutput::Ready(List(l)))
        }
        Map(m) if m.contains_meta_key(&WriteOp::IndexAssign.into()) => {
            let index_op = m.get_meta_value(&WriteOp::IndexAssign.into()).unwrap();
            let arg = Map(m);

            ctx.run_with_vm(move |mut vm| async move {
                match vm.run_unary_op(UnaryOp::Size, arg.clone()).await? {
                    Number(size) => {
                        if size <= 0 {
                            return runtime_error!("expected a positive @size, found {}", size);
                        }

                        for i in (1..usize::from(size)).rev() {
                            let j = rng.random_index(i + 1)?;
                            if i == j {
                                continue;
                            }

                            let value_i =
                                vm.run_read_op(ReadOp::Index, arg.clone(), i.into()).await?;
                            let value_j =
                                vm.run_read_op(ReadOp::Index, arg.clone(), j.into()).await?;

                            vm.call_instance_function_with_args(
                                arg.clone(),
                                index_op.clone(),
                                vec![i.into(), value_j],
                            )
                            .await?;

                            vm.call_instance_function_with_args(
                                arg.clone(),
                                index_op.clone(),
                                vec![j.into(), value_i],
                            )
                            .await?;
                        }

                        Ok(arg)
                    }
                    unexpected => unexpected_type("a Number from @size", &unexpected),
                }
            })
        }
        Map(m) => {
            {
                let mut data = m.data_mut();
                for i in (1..data.len()).rev() {
                    let j = rng.random_index(i + 1)?;
                    data.swap_indices(i, j);
                }
            }

            Ok(FunctionOutput::Ready(Map(m)))
        }
        Object(o) => {
            {
                let mut o_borrow = o.try_borrow_mut()?;
                let Some(size) = o_borrow.size() else {
                    return runtime_error!("{} has an unknown size", o_borrow.type_string())
                        .map(FunctionOutput::Ready);
                };

                for i in (1..size).rev() {
                    let j = rng.random_index(i + 1)?;
                    if i == j {
                        continue;
                    }
                    let i = KValue::from(i);
                    let j = KValue::from(j);
                    let value_i = o_borrow.index(&i)?;
                    let value_j = o_borrow.index(&j)?;
                    o_borrow.index_assign(&i, &value_j)?;
                    o_borrow.index_assign(&j, &value_i)?;
                }
            }

            Ok(FunctionOutput::Ready(Object(o)))
        }
        unexpected => unexpected_type("|Indexable|", &unexpected).map(FunctionOutput::Ready),
    }
}

thread_local! {
    static THREAD_RNG: RefCell<Xoshiro256PlusPlusRng>
        = RefCell::new(Xoshiro256PlusPlusRng(Xoshiro256PlusPlus::from_os_rng()));
}
