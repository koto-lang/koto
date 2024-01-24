//! The `koto` core library module

use crate::prelude::*;
use koto_bytecode::Chunk;
use koto_memory::Ptr;
use std::hash::{Hash, Hasher};

use super::io::File;

/// Initializes the `koto` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.koto");

    result.insert("args", KValue::Tuple(KTuple::default()));

    result.add_fn("copy", |ctx| match ctx.args() {
        [KValue::Iterator(iter)] => Ok(iter.make_copy()?.into()),
        [KValue::List(l)] => Ok(KList::with_data(l.data().clone()).into()),
        [KValue::Map(m)] => {
            let result = KMap::with_contents(
                m.data().clone(),
                m.meta_map().map(|meta| meta.borrow().clone()),
            );
            Ok(result.into())
        }
        [KValue::Object(o)] => o.try_borrow().map(|o| o.copy().into()),
        [other] => Ok(other.clone()),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_fn("deep_copy", |ctx| match ctx.args() {
        [value] => value.deep_copy(),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_fn("exports", |ctx| Ok(KValue::Map(ctx.vm.exports().clone())));

    result.add_fn("hash", |ctx| match ctx.args() {
        [value] => match ValueKey::try_from(value.clone()) {
            Ok(key) => {
                let mut hasher = KotoHasher::default();
                key.hash(&mut hasher);
                Ok(hasher.finish().into())
            }
            Err(_) => Ok(KValue::Null),
        },
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.insert("script_dir", KValue::Null);
    result.insert("script_path", KValue::Null);

    result.add_fn("type", |ctx| match ctx.args() {
        [value] => Ok(value.type_as_string().into()),
        unexpected => type_error_with_slice("a single argument", unexpected),
    });

    result.add_fn("load", |ctx| match ctx.args() {
        [KValue::Str(s)] => try_load_koto_script(ctx, s),
        [KValue::Object(o)] if o.is_a::<File>() => {
            let mut file = o.cast_mut::<File>().unwrap();
            let contents = file.read_to_kstring()?;
            try_load_koto_script(ctx, &contents)
        }
        unexpected => type_error_with_slice("a single String or io.File argument", unexpected),
    });

    result.add_fn("run", |ctx| match ctx.args() {
        [KValue::Str(s)] => match try_compile_koto_script(ctx, s) {
            Ok(chunk) => ctx.vm.run(Ptr::clone(&chunk)),
            Err(err) => runtime_error!(err.to_string()),
        },
        [KValue::Object(o)] if o.is_a::<File>() => {
            let mut file = o.cast_mut::<File>().unwrap();
            let contents = file.read_to_kstring()?;
            drop(file);
            match try_compile_koto_script(ctx, &contents) {
                Ok(chunk) => ctx.vm.run(Ptr::clone(&chunk)),
                Err(err) => runtime_error!(err.to_string()),
            }
        }
        [KValue::Map(m)] if is_chunk(m.get_meta_value(&MetaKey::Type)) => {
            let f = m.data().get("run").unwrap().clone();
            ctx.vm.run_function(f, CallArgs::None)
        }
        unexpected => {
            type_error_with_slice("a single String, io.File or chunk argument", unexpected)
        }
    });

    result
}

const CHUNK_TYPE_NAME: &str = "Chunk";

fn is_chunk(maybe_type_name: Option<KValue>) -> bool {
    if let Some(type_name) = maybe_type_name {
        matches!(type_name, KValue::Str(s) if s == CHUNK_TYPE_NAME)
    } else {
        false
    }
}

fn try_load_koto_script(
    ctx: &CallContext<'_>,
    script: &KString,
) -> Result<KValue, crate::error::Error> {
    let mut result = KMap::with_type(CHUNK_TYPE_NAME);

    match try_compile_koto_script(ctx, script) {
        Ok(chunk) => {
            result.insert("ok", true);
            result.add_fn("run", move |ctx| match ctx.vm.run(Ptr::clone(&chunk)) {
                Ok(value) => Ok(value),
                Err(err) => Ok(KString::from(err.to_string()).into()),
            });
        }
        Err(err) => {
            result.insert("ok", false);
            result.insert_meta(
                MetaKey::UnaryOp(UnaryOp::Display),
                KValue::NativeFunction(KNativeFunction::new(move |_| {
                    Ok(KString::from(err.to_string()).into())
                })),
            );
        }
    }

    Ok(result.into())
}

fn try_compile_koto_script(
    ctx: &CallContext<'_>,
    script: &KString,
) -> Result<Ptr<Chunk>, koto_bytecode::LoaderError> {
    ctx.vm.loader().borrow_mut().compile_script(
        script,
        &None,
        koto_bytecode::CompilerSettings::default(),
    )
}
