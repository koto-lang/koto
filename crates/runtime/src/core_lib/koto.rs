//! The `koto` core library module
use super::io::File;
use crate::prelude::*;
use crate::Result;
use koto_derive::{koto_impl, koto_method, KotoCopy, KotoType};
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
            let file = o.cast::<File>().unwrap();
            let contents = file.inner().read_to_string()?;
            try_load_koto_script(ctx, &contents)
        }
        unexpected => type_error_with_slice("a single String or io.File argument", unexpected),
    });

    result.add_fn("run", |ctx| match ctx.args() {
        [KValue::Str(s)] => match try_compile_koto_script(ctx, s) {
            Ok(chunk_ptr) => try_run_chunk(ctx.vm, chunk_ptr),
            Err(err) => Ok(RunResult::from(RunResultInner::Err(err.to_string())).into()),
        },
        [KValue::Object(o)] if o.is_a::<File>() => {
            let file = o.cast::<File>().unwrap();
            let contents = file.inner().read_to_string()?;
            drop(file);
            match try_compile_koto_script(ctx, &contents) {
                Ok(chunk_ptr) => try_run_chunk(ctx.vm, chunk_ptr),
                Err(err) => Ok(RunResult::from(RunResultInner::Err(err.to_string())).into()),
            }
        }
        [KValue::Object(o)] if o.is_a::<Chunk>() => {
            let chunk = o.cast::<Chunk>().unwrap();
            match &chunk.0 {
                ChunkInner::Ok(chunk_ptr) => {
                    let chunk_ptr = Ptr::clone(chunk_ptr);
                    drop(chunk);
                    try_run_chunk(ctx.vm, chunk_ptr)
                }
                ChunkInner::Err(err) => {
                    Ok(RunResult::from(RunResultInner::Err(err.to_string())).into())
                }
            }
        }
        unexpected => {
            type_error_with_slice("a single String, io.File or Chunk argument", unexpected)
        }
    });

    result
}

fn try_load_koto_script(ctx: &CallContext<'_>, script: &str) -> Result<KValue> {
    match try_compile_koto_script(ctx, script) {
        Ok(chunk) => Ok(Chunk::from(ChunkInner::Ok(Ptr::clone(&chunk))).into()),
        Err(err) => Ok(Chunk::from(ChunkInner::Err(err.to_string())).into()),
    }
}

fn try_run_chunk(vm: &mut KotoVm, chunk_ptr: Ptr<koto_bytecode::Chunk>) -> Result<KValue> {
    match vm.run(chunk_ptr) {
        Ok(result) => Ok(RunResult::from(RunResultInner::Ok(result)).into()),
        Err(err) => Ok(RunResult::from(RunResultInner::Err(err.to_string())).into()),
    }
}

fn try_compile_koto_script(
    ctx: &CallContext<'_>,
    script: &str,
) -> core::result::Result<Ptr<koto_bytecode::Chunk>, koto_bytecode::LoaderError> {
    ctx.vm.loader().borrow_mut().compile_script(
        script,
        &None,
        koto_bytecode::CompilerSettings::default(),
    )
}

#[derive(Clone)]
enum ChunkInner {
    Ok(Ptr<koto_bytecode::Chunk>),
    Err(String),
}

/// The Chunk type used in the koto module
#[derive(Clone, KotoCopy, KotoType)]
pub struct Chunk(ChunkInner);

#[koto_impl(runtime = crate)]
impl Chunk {
    #[koto_method]
    fn ok(&self) -> KValue {
        matches!(self.0, ChunkInner::Ok(_)).into()
    }
}

impl KotoObject for Chunk {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        match &self.0 {
            ChunkInner::Ok(chunk) => {
                ctx.append(format!("{:?}", chunk));
            }
            ChunkInner::Err(err) => {
                ctx.append(err.to_owned());
            }
        }
        Ok(())
    }
}

impl From<ChunkInner> for Chunk {
    fn from(inner: ChunkInner) -> Self {
        Self(inner)
    }
}

impl From<Chunk> for KValue {
    fn from(chunk: Chunk) -> Self {
        KObject::from(chunk).into()
    }
}

#[derive(Clone)]
enum RunResultInner {
    Ok(KValue),
    Err(String),
}

/// The RunResult type used in the koto module
#[derive(Clone, KotoCopy, KotoType)]
pub struct RunResult(RunResultInner);

#[koto_impl(runtime = crate)]
impl RunResult {
    #[koto_method]
    fn ok(&self) -> KValue {
        matches!(self.0, RunResultInner::Ok(_)).into()
    }

    #[koto_method]
    fn value(&self) -> KValue {
        match &self.0 {
            RunResultInner::Ok(value) => value.clone(),
            RunResultInner::Err(value) => value.to_owned().into(),
        }
    }
}

impl KotoObject for RunResult {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        match &self.0 {
            RunResultInner::Ok(value) => {
                ctx.append(format!("{:?}", value));
            }
            RunResultInner::Err(err) => {
                ctx.append(err.to_owned());
            }
        }
        Ok(())
    }
}

impl From<RunResultInner> for RunResult {
    fn from(inner: RunResultInner) -> Self {
        Self(inner)
    }
}

impl From<RunResult> for KValue {
    fn from(result: RunResult) -> Self {
        KObject::from(result).into()
    }
}
