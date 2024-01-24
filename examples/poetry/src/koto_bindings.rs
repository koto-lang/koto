use crate::Poetry;
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
            unexpected => type_error_with_slice("a String", unexpected),
        }
    });

    result
}

#[derive(Clone, KotoCopy, KotoType)]
#[koto(type_name = "Poetry")]
struct KotoPoetry(Poetry);

impl KotoLookup for KotoPoetry {}

impl KotoObject for KotoPoetry {
    fn is_iterable(&self) -> IsIterable {
        IsIterable::ForwardIterator
    }

    fn iterator_next(&mut self, _vm: &mut Vm) -> Option<KIteratorOutput> {
        self.0
            .next_word()
            .map(|word| KIteratorOutput::Value(word.as_ref().into()))
    }
}
