use crate::Poetry;
use koto::runtime::{Backend, Result};
use koto::{derive::*, prelude::*};

pub fn make_module() -> KMap {
    let result = KMap::with_type("poetry");

    result.add_fn("new", {
        |ctx| match ctx.args() {
            [KValue::Str(text)] => {
                let mut poetry = Poetry::default();
                poetry.add_source_material(text);
                Ok(KObject::from(KotoPoetry(poetry)).into())
            }
            unexpected => unexpected_args("|String|", unexpected),
        }
    });

    result
}

#[derive(Clone, KotoCopy, KotoType)]
#[koto(type_name = "Poetry")]
struct KotoPoetry(Poetry);

impl<B: KotoBackend> KotoAccess<B> for KotoPoetry {}

impl KotoObjectOps<Backend> for KotoPoetry {
    fn is_iterable(&self) -> Result<IsIterable> {
        Ok(IsIterable::ForwardIterator)
    }

    fn iterator_next(&mut self, _vm: &mut KotoVm) -> Result<Option<KIteratorOutput>> {
        Ok(self
            .0
            .next_word()
            .map(|word| KIteratorOutput::Value(word.as_ref().into())))
    }
}
