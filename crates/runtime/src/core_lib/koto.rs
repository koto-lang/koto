//! The `koto` core library module

use crate::Result;
use crate::prelude::*;
use koto_bytecode::CompilerSettings;
use koto_derive::{KotoCopy, KotoType};
use koto_memory::Ptr;
use std::{
    hash::{Hash, Hasher},
    path::Path,
};

/// Initializes the `koto` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.koto");

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

    result.add_fn("script_dir", |ctx| {
        let result = match &ctx.vm.chunk().path {
            Some(script_path) => Path::new(script_path.as_str())
                .parent()
                .and_then(|parent| parent.to_str())
                .and_then(|parent| script_path.with_bounds(0..parent.len()))
                .into(),
            None => KValue::Null,
        };
        Ok(result)
    });

    result.add_fn("script_path", |ctx| {
        let result = match &ctx.vm.chunk().path {
            Some(path) => KValue::from(path.clone()),
            None => KValue::Null,
        };
        Ok(result)
    });

    result.add_vm_fn("size", |ctx| match ctx.args() {
        [value] => {
            let value = value.clone();
            ctx.run_with_vm(|mut vm| async move { vm.run_unary_op(UnaryOp::Size, value).await })
        }
        unexpected => unexpected_args::<KValue>("|Any|", unexpected).map(FunctionOutput::Ready),
    });

    result.add_fn("type", |ctx| match ctx.args() {
        [value] => Ok(value.type_as_string().into()),
        unexpected => unexpected_args("|Any|", unexpected),
    });

    result.insert("unimplemented", KObject::from(Unimplemented));

    result.add_fn("load", |ctx| {
        let script = match ctx.args() {
            [KValue::Str(s)] => s.clone(),
            unexpected => return unexpected_args("|String|", unexpected),
        };

        Ok(try_load_koto_script(ctx, script.as_str())?.into())
    });

    result.add_vm_fn("run", |ctx| {
        let chunk = match ctx.args() {
            [KValue::Str(s)] => {
                let mut vm = ctx.spawn_shared_vm();
                try_load_koto_script_with_vm(&mut vm, s)?.inner()
            }
            [KValue::Object(o)] if let Ok(chunk) = o.cast::<Chunk>().map(|chunk| chunk.inner()) => {
                chunk
            }
            unexpected => {
                return unexpected_args::<KValue>("|String|, or |Chunk|", unexpected)
                    .map(FunctionOutput::Ready);
            }
        };

        ctx.run_with_vm(|mut vm| async move { vm.run(chunk).await })
    });

    result
}

fn try_load_koto_script(ctx: &mut CallContext<'_>, script: &str) -> Result<Chunk> {
    try_load_koto_script_with_vm(ctx.vm, script)
}

fn try_load_koto_script_with_vm(vm: &mut KotoVm, script: &str) -> Result<Chunk> {
    let chunk =
        vm.loader()
            .borrow_mut()
            .compile_script(script, None, CompilerSettings::default())?;

    Ok(chunk.into())
}

/// The Chunk type used in the koto module
#[derive(Clone, KotoCopy, KotoType)]
#[koto(runtime = crate)]
pub struct Chunk(Ptr<koto_bytecode::Chunk>);

impl Chunk {
    fn inner(&self) -> Ptr<koto_bytecode::Chunk> {
        Ptr::clone(&self.0)
    }
}

impl KotoAccess for Chunk {}

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

/// A type error type used in the koto module
#[derive(Clone, KotoCopy, KotoType)]
#[koto(runtime = crate)]
pub struct Unimplemented;

impl KotoAccess for Unimplemented {}
impl KotoObject for Unimplemented {}
