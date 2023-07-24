//! A double-ended peekable iterator for Koto

use {super::iter_output_to_result, crate::prelude::*};

/// A double-ended peekable iterator for Koto
#[derive(Clone, Debug)]
pub struct Peekable {
    iter: ValueIterator,
    peeked_front: Option<Value>,
    peeked_back: Option<Value>,
}

impl ExternalData for Peekable {
    fn data_type(&self) -> ValueString {
        PEEKABLE_TYPE_STRING.with(|x| x.clone())
    }

    fn make_copy(&self) -> PtrMut<dyn ExternalData> {
        make_data_ptr(self.clone())
    }
}

impl Peekable {
    /// Initializes a Peekable that wraps the given iterator
    pub fn new(iter: ValueIterator) -> Self {
        Self {
            iter,
            peeked_front: None,
            peeked_back: None,
        }
    }

    /// Makes an instance of Peekable along with a meta map that allows it be used as a Koto Value
    pub fn make_value(iter: ValueIterator) -> Value {
        Value::External(External::with_shared_meta_map(
            Self::new(iter),
            PEEKABLE_META.with(|meta| meta.clone()),
        ))
    }

    fn peek(&mut self) -> RuntimeResult {
        match self.peeked_front.clone() {
            Some(peeked) => Ok(peeked),
            None => match self.next()? {
                Value::Null => Ok(Value::Null),
                peeked => {
                    self.peeked_front = Some(peeked.clone());
                    Ok(peeked)
                }
            },
        }
    }

    fn peek_back(&mut self) -> RuntimeResult {
        match self.peeked_back.clone() {
            Some(peeked) => Ok(peeked),
            None => match self.next_back()? {
                Value::Null => Ok(Value::Null),
                peeked => {
                    self.peeked_back = Some(peeked.clone());
                    Ok(peeked)
                }
            },
        }
    }

    fn next(&mut self) -> RuntimeResult {
        match self.peeked_front.take() {
            Some(value) => Ok(value),
            None => match iter_output_to_result(self.iter.next())? {
                Value::Null => Ok(self.peeked_back.take().unwrap_or(Value::Null)),
                other => Ok(other),
            },
        }
    }

    fn next_back(&mut self) -> RuntimeResult {
        match self.peeked_back.take() {
            Some(value) => Ok(value),
            None => match iter_output_to_result(self.iter.next_back())? {
                Value::Null => Ok(self.peeked_front.take().unwrap_or(Value::Null)),
                other => Ok(other),
            },
        }
    }
}

fn make_peekable_meta_map() -> PtrMut<MetaMap> {
    use UnaryOp::*;

    MetaMapBuilder::<Peekable>::new(PEEKABLE_TYPE)
        .function("peek", |context| context.data_mut()?.peek())
        .function("peek_back", |context| context.data_mut()?.peek_back())
        .function(Next, |context| context.data_mut()?.next())
        .function(NextBack, |context| context.data_mut()?.next_back())
        .build()
}

const PEEKABLE_TYPE: &str = "Peekable";

thread_local! {
    static PEEKABLE_META: PtrMut<MetaMap> = make_peekable_meta_map();
    static PEEKABLE_TYPE_STRING: ValueString = PEEKABLE_TYPE.into();
}
