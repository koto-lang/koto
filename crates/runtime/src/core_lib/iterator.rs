//! The `iterator` core library module

pub mod adaptors;
pub mod generators;
pub mod peekable;

use crate::{KIteratorOutput as Output, Result, derive::*, prelude::*};

static MODULE_NAME: &str = "core.iterator";

/// Initializes the `iterator` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type(MODULE_NAME);

    result.add_vm_fn("advance", |ctx| {
        let expected_error = "|Iterator, Number >= 0|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (KValue::Iterator(iterator), [KValue::Number(n)]) if *n >= 0.0 => {
                let mut iterator = iterator.clone();
                let mut remaining = usize::from(n);

                ctx.run_with_vm(|mut vm| async move {
                    while remaining > 0 {
                        match vm.next(&mut iterator).await? {
                            Some(Output::Error(error)) => return Err(error),
                            Some(_) => remaining -= 1,
                            None => break,
                        }
                    }

                    Ok(remaining.into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("all", |ctx| {
        let expected_error = "|Iterable, |Any| -> Bool|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [predicate]) if predicate.is_callable() => {
                let iterable = iterable.clone();
                let predicate = predicate.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match call_function_with_output(&mut vm, predicate.clone(), output).await? {
                            KValue::Bool(result) => {
                                if !result {
                                    return Ok(false.into());
                                }
                            }
                            unexpected => {
                                return unexpected_type(
                                    "a Bool to be returned from the predicate",
                                    &unexpected,
                                );
                            }
                        }
                    }

                    Ok(true.into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("any", |ctx| {
        let expected_error = "|Iterable, |Any| -> Bool|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [predicate]) if predicate.is_callable() => {
                let iterable = iterable.clone();
                let predicate = predicate.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match call_function_with_output(&mut vm, predicate.clone(), output).await? {
                            KValue::Bool(result) => {
                                if result {
                                    return Ok(true.into());
                                }
                            }
                            unexpected => {
                                return unexpected_type(
                                    "a Bool to be returned from the predicate",
                                    &unexpected,
                                );
                            }
                        }
                    }

                    Ok(false.into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("chain", |ctx| {
        let expected_error = "|Iterable, Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable_a, [iterable_b]) if iterable_b.is_iterable() => {
                let iterable_a = iterable_a.clone();
                let iterable_b = iterable_b.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let result = KIterator::new(adaptors::Chain::new(
                        vm.make_iterator(iterable_a).await?,
                        vm.make_iterator(iterable_b).await?,
                    ));

                    Ok(KValue::Iterator(result))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("chunks", |ctx| {
        let expected_error = "|Iterable, Number|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [KValue::Number(n)]) => {
                let iterable = iterable.clone();
                let n = *n;

                ctx.run_with_vm(|mut vm| async move {
                    match adaptors::Chunks::new(vm.make_iterator(iterable).await?, n.into()) {
                        Ok(result) => Ok(KIterator::new(result).into()),
                        Err(e) => runtime_error!("iterator.chunks: {}", e),
                    }
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("consume", |ctx| {
        let expected_error = "|Iterable|, or |Iterable, |Any| -> Any|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;

                    while let Some(output) = vm.next(&mut iterator).await? {
                        if let Output::Error(error) = output {
                            return Err(error);
                        }
                    }

                    Ok(KValue::Null)
                })
            }
            (iterable, [f]) if f.is_callable() => {
                let iterable = iterable.clone();
                let f = f.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;

                    while let Some(output) = vm.next(&mut iterator).await? {
                        call_function_with_output(&mut vm, f.clone(), output).await?;
                    }

                    Ok(KValue::Null)
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("count", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let mut result = 0;

                    while let Some(output) = vm.next(&mut iterator).await? {
                        if let Output::Error(error) = output {
                            return Err(error);
                        }
                        result += 1;
                    }

                    Ok(KValue::Number(result.into()))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("cycle", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let result = adaptors::Cycle::new(vm.make_iterator(iterable).await?);
                    Ok(KIterator::new(result).into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("each", |ctx| {
        let expected_error = "|Iterable, |Any| -> Any|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [f]) if f.is_callable() => {
                let iterable = iterable.clone();
                let f = f.clone();
                let adaptor_vm = ctx.spawn_shared_vm();

                ctx.run_with_vm(|mut vm| async move {
                    let result =
                        adaptors::Each::new(vm.make_iterator(iterable).await?, f, &adaptor_vm);

                    Ok(KIterator::new(result).into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("enumerate", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let result = adaptors::Enumerate::new(vm.make_iterator(iterable).await?);
                    Ok(KIterator::new(result).into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("find", |ctx| {
        let expected_error = "|Iterable, |Any| -> Bool|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [predicate]) if predicate.is_callable() => {
                let iterable = iterable.clone();
                let predicate = predicate.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match collect_pair(output) {
                            Output::Value(value) => {
                                match vm
                                    .call_function_with_arg(predicate.clone(), value.clone())
                                    .await?
                                {
                                    KValue::Bool(result) => {
                                        if result {
                                            return Ok(value);
                                        }
                                    }
                                    unexpected => {
                                        return unexpected_type(
                                            "a Bool to be returned from the predicate",
                                            &unexpected,
                                        );
                                    }
                                }
                            }
                            Output::Error(error) => return Err(error),
                            _ => unreachable!(),
                        }
                    }

                    Ok(KValue::Null)
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("flatten", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();
                let adaptor_vm = ctx.spawn_shared_vm();

                ctx.run_with_vm(|mut vm| async move {
                    let result =
                        adaptors::Flatten::new(vm.make_iterator(iterable).await?, &adaptor_vm);

                    Ok(KIterator::new(result).into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("fold", |ctx| {
        let expected_error = "|Iterable, Any, |Any, Any| -> Any|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [result, f]) if f.is_callable() => {
                let iterable = iterable.clone();
                let result = result.clone();
                let f = f.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let mut fold_result = result;

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match collect_pair(output) {
                            Output::Value(value) => {
                                fold_result = vm
                                    .call_function_with_args(f.clone(), vec![fold_result, value])
                                    .await?;
                            }
                            Output::Error(error) => return Err(error),
                            _ => unreachable!(),
                        }
                    }

                    Ok(fold_result)
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_fn("generate", |ctx| {
        let instance = ctx.instance();
        if !(matches!(instance, &KValue::Null) || instance.type_as_string() == MODULE_NAME) {
            return runtime_error!("iterator.generate must be used as a standalone function");
        }

        match ctx.args() {
            [f] if f.is_callable() => {
                let result = generators::Generate::new(f.clone(), ctx.vm);
                Ok(KIterator::new(result).into())
            }
            [f, KValue::Number(n)] if *n >= 0 && f.is_callable() => {
                let result = generators::GenerateN::new(n.into(), f.clone(), ctx.vm);
                Ok(KIterator::new(result).into())
            }
            unexpected => unexpected_args(
                "|generator: || -> Any|, or |generator: || -> Any, n: Number >= 0|",
                unexpected,
            ),
        }
    });

    result.add_vm_fn("intersperse", |ctx| {
        let expected_error = "|Iterable, Value|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [separator_fn]) if separator_fn.is_callable() => {
                let iterable = iterable.clone();
                let separator_fn = separator_fn.clone();
                let adaptor_vm = ctx.spawn_shared_vm();

                ctx.run_with_vm(|mut vm| async move {
                    let result = adaptors::IntersperseWith::new(
                        vm.make_iterator(iterable).await?,
                        separator_fn,
                        &adaptor_vm,
                    );

                    Ok(KIterator::new(result).into())
                })
            }
            (iterable, [separator]) => {
                let iterable = iterable.clone();
                let separator = separator.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let result =
                        adaptors::Intersperse::new(vm.make_iterator(iterable).await?, separator);

                    Ok(KIterator::new(result).into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("iter", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();
                ctx.run_with_vm(|mut vm| async move {
                    Ok(KValue::Iterator(vm.make_iterator(iterable).await?))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("keep", |ctx| {
        let expected_error = "|Iterable, |Any| -> Bool|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [predicate]) if predicate.is_callable() => {
                let iterable = iterable.clone();
                let predicate = predicate.clone();
                let adaptor_vm = ctx.spawn_shared_vm();

                ctx.run_with_vm(|mut vm| async move {
                    let result = adaptors::Keep::new(
                        vm.make_iterator(iterable).await?,
                        predicate,
                        &adaptor_vm,
                    );

                    Ok(KIterator::new(result).into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("last", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let mut result = KValue::Null;

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match collect_pair(output) {
                            Output::Value(value) => result = value,
                            Output::Error(error) => return Err(error),
                            _ => unreachable!(),
                        }
                    }

                    Ok(result)
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("max", |ctx| {
        let expected_error = "|Iterable|, or |Iterable, |Any| -> Any|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();
                ctx.run_with_vm(|mut vm| async move {
                    run_iterator_comparison(&mut vm, iterable, InvertResult::Yes).await
                })
            }
            (iterable, [key_fn]) if key_fn.is_callable() => {
                let iterable = iterable.clone();
                let key_fn = key_fn.clone();
                ctx.run_with_vm(|mut vm| async move {
                    run_iterator_comparison_by_key(&mut vm, iterable, key_fn, InvertResult::Yes)
                        .await
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("min", |ctx| {
        let expected_error = "|Iterable|, or |Iterable, |Any| -> Any|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();
                ctx.run_with_vm(|mut vm| async move {
                    run_iterator_comparison(&mut vm, iterable, InvertResult::No).await
                })
            }
            (iterable, [key_fn]) if key_fn.is_callable() => {
                let iterable = iterable.clone();
                let key_fn = key_fn.clone();
                ctx.run_with_vm(|mut vm| async move {
                    run_iterator_comparison_by_key(&mut vm, iterable, key_fn, InvertResult::No)
                        .await
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("min_max", |ctx| {
        let expected_error = "|Iterable|, or |Iterable, |Any| -> Any|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let mut result = None;

                    while let Some(iter_output) = vm.next(&mut iterator).await? {
                        match collect_pair(iter_output) {
                            Output::Value(value) => {
                                result = Some(match result {
                                    Some((min, max)) => (
                                        compare_values(
                                            &mut vm,
                                            min,
                                            value.clone(),
                                            InvertResult::No,
                                        )
                                        .await?,
                                        compare_values(&mut vm, max, value, InvertResult::Yes)
                                            .await?,
                                    ),
                                    None => (value.clone(), value),
                                })
                            }
                            Output::Error(error) => return Err(error),
                            _ => unreachable!(),
                        }
                    }

                    Ok(result.map_or(KValue::Null, |(min, max)| {
                        KValue::Tuple(vec![min, max].into())
                    }))
                })
            }
            (iterable, [key_fn]) if key_fn.is_callable() => {
                let iterable = iterable.clone();
                let key_fn = key_fn.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let mut result = None;

                    while let Some(iter_output) = vm.next(&mut iterator).await? {
                        match collect_pair(iter_output) {
                            Output::Value(value) => {
                                let key = vm
                                    .call_function_with_arg(key_fn.clone(), value.clone())
                                    .await?;
                                let value_and_key = (value, key);

                                result = Some(match result {
                                    Some((min_and_key, max_and_key)) => (
                                        compare_values_with_key(
                                            &mut vm,
                                            min_and_key,
                                            value_and_key.clone(),
                                            InvertResult::No,
                                        )
                                        .await?,
                                        compare_values_with_key(
                                            &mut vm,
                                            max_and_key,
                                            value_and_key,
                                            InvertResult::Yes,
                                        )
                                        .await?,
                                    ),
                                    None => (value_and_key.clone(), value_and_key),
                                })
                            }
                            Output::Error(error) => return Err(error),
                            _ => unreachable!(), // value pairs have been collected in collect_pair
                        }
                    }

                    Ok(result.map_or(KValue::Null, |((min, _), (max, _))| {
                        KValue::Tuple(vec![min, max].into())
                    }))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("next", |ctx| {
        let expected_error = "|Iterable|";

        let iter = match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (KValue::Iterator(i), []) => i.clone(),
            (iterable, []) if iterable.is_iterable() => {
                let iterable = iterable.clone();
                return ctx.run_with_vm(|mut vm| async move {
                    let mut iter = vm.make_iterator(iterable).await?;
                    let output = match iter_output_to_result(vm.next(&mut iter).await?)? {
                        None => KValue::Null,
                        Some(output) => IteratorOutput::from(output).into(),
                    };

                    Ok(output)
                });
            }
            (instance, args) => {
                return unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready);
            }
        };

        ctx.run_with_vm(|mut vm| async move {
            let mut iter = iter;
            let output = match iter_output_to_result(vm.next(&mut iter).await?)? {
                None => KValue::Null,
                Some(output) => IteratorOutput::from(output).into(),
            };

            Ok(output)
        })
    });

    result.add_vm_fn("next_back", |ctx| {
        let expected_error = "|Iterable|";

        let mut iter = match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (KValue::Iterator(i), []) => i.clone(),
            (iterable, []) if iterable.is_iterable() => {
                let iterable = iterable.clone();
                return ctx.run_with_vm(|mut vm| async move {
                    let mut iter = vm.make_iterator(iterable).await?;
                    let output = match iter_output_to_result(vm.next_back(&mut iter).await?)? {
                        None => KValue::Null,
                        Some(output) => IteratorOutput::from(output).into(),
                    };

                    Ok(output)
                });
            }
            (instance, args) => {
                return unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready);
            }
        };

        ctx.run_with_vm(|mut vm| async move {
            let output = match iter_output_to_result(vm.next_back(&mut iter).await?)? {
                None => KValue::Null,
                Some(output) => IteratorOutput::from(output).into(),
            };

            Ok(output)
        })
    });

    result.add_fn("once", |ctx| {
        let instance = ctx.instance();
        if !(matches!(instance, &KValue::Null) || instance.type_as_string() == MODULE_NAME) {
            return runtime_error!("iterator.once must be used as a standalone function");
        }

        match ctx.args() {
            [value] => Ok(KIterator::new(generators::Once::new(value.clone())).into()),
            unexpected => unexpected_args("|Any|", unexpected),
        }
    });

    result.add_vm_fn("peekable", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();
                ctx.run_with_vm(|mut vm| async move {
                    Ok(peekable::Peekable::make_value(
                        vm.make_iterator(iterable).await?,
                    ))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("position", |ctx| {
        let expected_error = "|Iterable, |Any| -> Bool|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [predicate]) if predicate.is_callable() => {
                let iterable = iterable.clone();
                let predicate = predicate.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let mut i = 0;

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match call_function_with_output(&mut vm, predicate.clone(), output).await? {
                            KValue::Bool(result) => {
                                if result {
                                    return Ok(i.into());
                                }
                            }
                            unexpected => {
                                return unexpected_type(
                                    "a Bool to be returned from the predicate",
                                    &unexpected,
                                );
                            }
                        }

                        i += 1;
                    }

                    Ok(KValue::Null)
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("product", |ctx| {
        let (iterable, initial_value) = {
            let expected_error = "|Iterable|";

            match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
                (iterable, []) => (iterable.clone(), KValue::Number(1.into())),
                (iterable, [initial_value]) => (iterable.clone(), initial_value.clone()),
                (instance, args) => {
                    return unexpected_args_after_instance::<KValue>(
                        expected_error,
                        instance,
                        args,
                    )
                    .map(FunctionOutput::Ready);
                }
            }
        };

        ctx.run_with_vm(|mut vm| async move {
            fold_with_operator(&mut vm, iterable, initial_value, BinaryOp::Multiply).await
        })
    });

    result.add_fn("repeat", |ctx| {
        let instance = ctx.instance();
        if !matches!(instance, &KValue::Null) && instance.type_as_string() != MODULE_NAME {
            return runtime_error!("iterator.repeat must be used as a standalone function");
        }

        match ctx.args() {
            [value] => {
                let result = generators::Repeat::new(value.clone());
                Ok(KIterator::new(result).into())
            }
            [value, KValue::Number(n)] if *n >= 0.0 => {
                let result = generators::RepeatN::new(value.clone(), n.into());
                Ok(KIterator::new(result).into())
            }
            unexpected => unexpected_args("|Any|, or |Any, Number >= 0|", unexpected),
        }
    });

    result.add_vm_fn("reversed", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    match adaptors::Reversed::new(vm.make_iterator(iterable).await?) {
                        Ok(result) => Ok(KIterator::new(result).into()),
                        Err(e) => runtime_error!("iterator.reversed: {}", e),
                    }
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("skip", |ctx| {
        let expected_error = "|Iterable, Number >= 0|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [KValue::Number(n)]) if *n >= 0.0 => {
                let iterable = iterable.clone();
                let n = *n;

                ctx.run_with_vm(|mut vm| async move {
                    let result = adaptors::Skip::new(vm.make_iterator(iterable).await?, n.into());
                    Ok(KIterator::new(result).into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("step", |ctx| {
        let expected_error = "|Iterable, Number|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [KValue::Number(n)]) => {
                if *n > 0 {
                    let iterable = iterable.clone();
                    let step_size = n.into();
                    ctx.run_with_vm(|mut vm| async move {
                        match adaptors::Step::new(vm.make_iterator(iterable).await?, step_size) {
                            Ok(result) => Ok(KIterator::new(result).into()),
                            Err(e) => runtime_error!("iterator.step: {}", e),
                        }
                    })
                } else {
                    runtime_error!("expected a non-negative number").map(FunctionOutput::Ready)
                }
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("sum", |ctx| {
        let (iterable, initial_value) = {
            let expected_error = "|Iterable|";

            match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
                (iterable, []) => (iterable.clone(), KValue::Number(0.into())),
                (iterable, [initial_value]) => (iterable.clone(), initial_value.clone()),
                (instance, args) => {
                    return unexpected_args_after_instance::<KValue>(
                        expected_error,
                        instance,
                        args,
                    )
                    .map(FunctionOutput::Ready);
                }
            }
        };

        ctx.run_with_vm(|mut vm| async move {
            fold_with_operator(&mut vm, iterable, initial_value, BinaryOp::Add).await
        })
    });

    result.add_vm_fn("take", |ctx| {
        let expected_error = "|Iterable, Number >= 0|, or |Iterable, |Any| -> Bool|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [KValue::Number(n)]) if *n >= 0.0 => {
                let iterable = iterable.clone();
                let n = *n;

                ctx.run_with_vm(|mut vm| async move {
                    let result = adaptors::Take::new(vm.make_iterator(iterable).await?, n.into());
                    Ok(KIterator::new(result).into())
                })
            }
            (iterable, [predicate]) if predicate.is_callable() => {
                let iterable = iterable.clone();
                let predicate = predicate.clone();
                let adaptor_vm = ctx.spawn_shared_vm();

                ctx.run_with_vm(|mut vm| async move {
                    let result = adaptors::TakeWhile::new(
                        vm.make_iterator(iterable).await?,
                        predicate,
                        &adaptor_vm,
                    );

                    Ok(KIterator::new(result).into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("to_list", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();
                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let (size_hint, _) = iterator.size_hint();
                    let mut result = ValueVec::with_capacity(size_hint);

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match collect_pair(output) {
                            Output::Value(value) => result.push(value),
                            Output::Error(error) => return Err(error),
                            _ => unreachable!(),
                        }
                    }

                    Ok(KValue::List(KList::with_data(result)))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("to_map", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let (size_hint, _) = iterator.size_hint();
                    let mut result = ValueMap::with_capacity(size_hint);

                    while let Some(output) = vm.next(&mut iterator).await? {
                        let (key, value) = match output {
                            Output::ValuePair(key, value) => (key, value),
                            Output::Value(KValue::Tuple(t)) if t.len() == 2 => {
                                let key = t[0].clone();
                                let value = t[1].clone();
                                (key, value)
                            }
                            Output::Value(value) => (value, KValue::Null),
                            Output::Error(error) => return Err(error),
                        };

                        result.insert(ValueKey::try_from(key)?, value);
                    }

                    Ok(KValue::Map(KMap::with_data(result)))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("to_string", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let (size_hint, _) = iterator.size_hint();
                    let mut result = String::with_capacity(size_hint);

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match collect_pair(output) {
                            Output::Value(KValue::Str(s)) => result.push_str(s.as_str()),
                            Output::Value(value) => {
                                result.push_str(&vm.value_to_string(value).await?)
                            }
                            Output::Error(error) => return Err(error),
                            _ => unreachable!(),
                        };
                    }

                    Ok(result.into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("to_tuple", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, []) => {
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let (size_hint, _) = iterator.size_hint();
                    let mut result = Vec::with_capacity(size_hint);

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match collect_pair(output) {
                            Output::Value(value) => result.push(value),
                            Output::Error(error) => return Err(error),
                            _ => unreachable!(),
                        }
                    }

                    Ok(KValue::Tuple(result.into()))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("windows", |ctx| {
        let expected_error = "|Iterable, Number|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable, [KValue::Number(n)]) => {
                let iterable = iterable.clone();
                let n = *n;

                ctx.run_with_vm(|mut vm| async move {
                    match adaptors::Windows::new(vm.make_iterator(iterable).await?, n.into()) {
                        Ok(result) => Ok(KIterator::new(result).into()),
                        Err(e) => runtime_error!("iterator.windows: {}", e),
                    }
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("zip", |ctx| {
        let expected_error = "|Iterable|";

        match ctx.instance_and_args(KValue::is_iterable, expected_error)? {
            (iterable_a, [iterable_b]) if iterable_b.is_iterable() => {
                let iterable_a = iterable_a.clone();
                let iterable_b = iterable_b.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let result = adaptors::Zip::new(
                        vm.make_iterator(iterable_a).await?,
                        vm.make_iterator(iterable_b).await?,
                    );
                    Ok(KIterator::new(result).into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result
}

pub(crate) fn collect_pair(iterator_output: Output) -> Output {
    match iterator_output {
        Output::ValuePair(first, second) => {
            Output::Value(KValue::Tuple(vec![first, second].into()))
        }
        _ => iterator_output,
    }
}

pub(crate) fn iter_output_to_result(iterator_output: Option<Output>) -> Result<Option<KValue>> {
    let output = match iterator_output {
        Some(Output::Value(value)) => Some(value),
        Some(Output::ValuePair(first, second)) => Some(KValue::Tuple(vec![first, second].into())),
        Some(Output::Error(error)) => return Err(error),
        None => None,
    };

    Ok(output)
}

async fn call_function_with_output(
    vm: &mut AsyncKotoVm,
    function: KValue,
    output: Output,
) -> Result<KValue> {
    match output {
        Output::Value(value) => vm.call_function_with_arg(function, value).await,
        Output::ValuePair(a, b) => vm.call_function_with_tuple(function, vec![a, b]).await,
        Output::Error(error) => Err(error),
    }
}

/// The output type used by operations like `iterator.next()` and `next_back()`
#[derive(Clone, KotoCopy, KotoType)]
#[koto(runtime = crate)]
pub struct IteratorOutput(KValue);

#[koto_impl(runtime = crate)]
impl IteratorOutput {
    /// Returns the wrapped output value
    #[koto_method]
    pub fn get(&self) -> KValue {
        self.0.clone()
    }
}

impl KotoObject for IteratorOutput {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(Self::type_static());
        ctx.append('(');

        let mut wrapped_ctx = DisplayContext::default();
        self.0.display(&mut wrapped_ctx)?;
        ctx.append(wrapped_ctx.result());

        ctx.append(')');
        Ok(())
    }
}

impl From<KValue> for IteratorOutput {
    fn from(value: KValue) -> Self {
        Self(value)
    }
}

impl From<IteratorOutput> for KValue {
    fn from(output: IteratorOutput) -> Self {
        KObject::from(output).into()
    }
}

async fn fold_with_operator(
    vm: &mut AsyncKotoVm,
    iterable: KValue,
    initial_value: KValue,
    operator: BinaryOp,
) -> Result<KValue> {
    let mut result = initial_value;
    let mut iterator = vm.make_iterator(iterable).await?;

    while let Some(output) = vm.next(&mut iterator).await? {
        match collect_pair(output) {
            Output::Value(rhs_value) => {
                result = vm.run_binary_op(operator, result, rhs_value).await?;
            }
            Output::Error(error) => return Err(error),
            _ => unreachable!(),
        }
    }

    Ok(result)
}

async fn run_iterator_comparison(
    vm: &mut AsyncKotoVm,
    iterable: KValue,
    invert_result: InvertResult,
) -> Result<KValue> {
    let mut result: Option<KValue> = None;
    let mut iterator = vm.make_iterator(iterable).await?;

    while let Some(iter_output) = vm.next(&mut iterator).await? {
        match collect_pair(iter_output) {
            Output::Value(value) => {
                result = Some(match result {
                    Some(result) => {
                        compare_values(vm, result.clone(), value.clone(), invert_result).await?
                    }
                    None => value,
                })
            }
            Output::Error(error) => return Err(error),
            _ => unreachable!(),
        }
    }

    Ok(result.unwrap_or_default())
}

async fn run_iterator_comparison_by_key(
    vm: &mut AsyncKotoVm,
    iterable: KValue,
    key_fn: KValue,
    invert_result: InvertResult,
) -> Result<KValue> {
    let mut result_and_key: Option<(KValue, KValue)> = None;
    let mut iterator = vm.make_iterator(iterable).await?;

    while let Some(iter_output) = vm.next(&mut iterator).await? {
        match collect_pair(iter_output) {
            Output::Value(value) => {
                let key = vm
                    .call_function_with_arg(key_fn.clone(), value.clone())
                    .await?;
                let value_and_key = (value, key);

                result_and_key = Some(match result_and_key {
                    Some(result_and_key) => {
                        compare_values_with_key(vm, result_and_key, value_and_key, invert_result)
                            .await?
                    }
                    None => value_and_key,
                });
            }
            Output::Error(error) => return Err(error),
            _ => unreachable!(),
        }
    }

    Ok(result_and_key.map_or(KValue::Null, |(value, _)| value))
}

// Compares two values using BinaryOp::Less
//
// Returns the lesser of the two values, unless `invert_result` is set to Yes
async fn compare_values(
    vm: &mut AsyncKotoVm,
    a: KValue,
    b: KValue,
    invert_result: InvertResult,
) -> Result<KValue> {
    use InvertResult::*;
    use KValue::Bool;

    let comparison_result = vm
        .run_binary_op(BinaryOp::Less, a.clone(), b.clone())
        .await?;

    match (comparison_result, invert_result) {
        (Bool(true), No) => Ok(a),
        (Bool(false), No) => Ok(b),
        (Bool(true), Yes) => Ok(b),
        (Bool(false), Yes) => Ok(a),
        (other, _) => runtime_error!(
            "Expected Bool from '<' comparison, found '{}'",
            other.type_as_string()
        ),
    }
}

// Compares two values using BinaryOp::Less
//
// Returns the lesser of the two values, unless `invert_result` is set to Yes
async fn compare_values_with_key(
    vm: &mut AsyncKotoVm,
    a_and_key: (KValue, KValue),
    b_and_key: (KValue, KValue),
    invert_result: InvertResult,
) -> Result<(KValue, KValue)> {
    use InvertResult::*;
    use KValue::Bool;

    let comparison_result = vm
        .run_binary_op(BinaryOp::Less, a_and_key.1.clone(), b_and_key.1.clone())
        .await?;

    match (comparison_result, invert_result) {
        (Bool(true), No) => Ok(a_and_key),
        (Bool(false), No) => Ok(b_and_key),
        (Bool(true), Yes) => Ok(b_and_key),
        (Bool(false), Yes) => Ok(a_and_key),
        (other, _) => runtime_error!(
            "Expected Bool from '<' comparison, found '{}'",
            other.type_as_string()
        ),
    }
}

#[derive(Clone, Copy)]
enum InvertResult {
    Yes,
    No,
}
