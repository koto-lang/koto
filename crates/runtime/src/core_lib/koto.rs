//! The `koto` core library module

use crate::prelude::*;
use crate::Result;
use koto_bytecode::CompilerSettings;
use koto_derive::{KotoCopy, KotoType};
use koto_memory::Ptr;
use std::hash::{Hash, Hasher};

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
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result.add_fn("deep_copy", |ctx| match ctx.args() {
        [value] => value.deep_copy(),
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result.add_fn("exports", |ctx| match ctx.args() {
        [] => Ok(KValue::Map(ctx.vm.exports().clone())),
        unexpected => unexpected_args("||", unexpected),
    });

    result.add_fn("hash", |ctx| match ctx.args() {
        [value] => match ValueKey::try_from(value.clone()) {
            Ok(key) => {
                let mut hasher = KotoHasher::default();
                key.hash(&mut hasher);
                Ok(hasher.finish().into())
            }
            Err(_) => Ok(KValue::Null),
        },
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result.insert("script_dir", KValue::Null);
    result.insert("script_path", KValue::Null);

    result.add_fn("size", |ctx| match ctx.args() {
        [value] => ctx.vm.run_unary_op(UnaryOp::Size, value.clone()),
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result.add_fn("type", |ctx| match ctx.args() {
        [value] => Ok(value.type_as_string().into()),
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result.add_fn("load", |ctx| match ctx.args() {
        [KValue::Str(s)] => Ok(try_load_koto_script(ctx, s)?.into()),
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("run", |ctx| match ctx.args() {
        [KValue::Str(s)] => {
            let chunk = try_load_koto_script(ctx, s)?;
            ctx.vm.run(chunk.inner())
        }
        [KValue::Object(o)] if o.is_a::<Chunk>() => {
            let chunk = o.cast::<Chunk>().unwrap().inner();
            ctx.vm.run(chunk)
        }
        unexpected => unexpected_args("|String|, or |Chunk|", unexpected),
    });

    result
}

fn try_load_koto_script(ctx: &CallContext<'_>, script: &str) -> Result<Chunk> {
    let chunk =
        ctx.vm
            .loader()
            .borrow_mut()
            .compile_script(script, None, CompilerSettings::default())?;

    Ok(chunk.into())
}

/// The Chunk type used in the koto module
#[derive(Clone, KotoCopy, KotoType)]
pub struct Chunk(Ptr<koto_bytecode::Chunk>);

impl Chunk {
    fn inner(&self) -> Ptr<koto_bytecode::Chunk> {
        Ptr::clone(&self.0)
    }
}

impl KotoEntries for Chunk {}

impl KotoObject for Chunk {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!(
            "{}({})",
            Self::type_static(),
            Ptr::address(&self.0)
        ));
        Ok(())
    }
}

impl From<Ptr<koto_bytecode::Chunk>> for Chunk {
    fn from(inner: Ptr<koto_bytecode::Chunk>) -> Self {
        Self(inner)
    }
}

impl From<Chunk> for KValue {
    fn from(chunk: Chunk) -> Self {
        KObject::from(chunk).into()
    }
}
