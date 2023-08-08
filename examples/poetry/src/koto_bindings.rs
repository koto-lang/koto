use crate::Poetry;
use koto::prelude::*;

pub fn make_module() -> ValueMap {
    let result = ValueMap::new();

    result.add_fn("new", {
        |vm, args| match vm.get_args(args) {
            [Value::Str(text)] => {
                let mut poetry = Poetry::default();
                poetry.add_source_material(text);
                Ok(poetry.into())
            }
            unexpected => type_error_with_slice("a String", unexpected),
        }
    });

    result
}

impl KotoType for Poetry {
    const TYPE: &'static str = "Poetry";
}

impl KotoObject for Poetry {
    fn object_type(&self) -> ValueString {
        Self::TYPE.into()
    }

    fn copy(&self) -> Object {
        self.clone().into()
    }

    fn is_iterable(&self) -> IsIterable {
        IsIterable::ForwardIterator
    }

    fn iterator_next(&mut self, _vm: &mut Vm) -> Option<ValueIteratorOutput> {
        self.next_word()
            .map(|word| ValueIteratorOutput::Value(word.as_ref().into()))
    }
}

impl From<Poetry> for Value {
    fn from(poetry: Poetry) -> Self {
        Object::from(poetry).into()
    }
}
