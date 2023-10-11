//! A double-ended peekable iterator for Koto

use super::iter_output_to_result;
use crate::{prelude::*, KIteratorOutput as Output, Result};

/// A double-ended peekable iterator for Koto
#[derive(Clone)]
pub struct Peekable {
    iter: KIterator,
    peeked_front: Option<Value>,
    peeked_back: Option<Value>,
}

impl Peekable {
    /// Initializes a Peekable that wraps the given iterator
    pub fn new(iter: KIterator) -> Self {
        Self {
            iter,
            peeked_front: None,
            peeked_back: None,
        }
    }

    /// Makes an instance of Peekable along with a meta map that allows it be used as a Koto Value
    pub fn make_value(iter: KIterator) -> Value {
        Object::from(Self::new(iter)).into()
    }

    fn peek(&mut self) -> Result<Value> {
        match self.peeked_front.clone() {
            Some(peeked) => Ok(peeked),
            None => match iter_output_to_result(self.next())? {
                Value::Null => Ok(Value::Null),
                peeked => {
                    self.peeked_front = Some(peeked.clone());
                    Ok(peeked)
                }
            },
        }
    }

    fn peek_back(&mut self) -> Result<Value> {
        match self.peeked_back.clone() {
            Some(peeked) => Ok(peeked),
            None => match iter_output_to_result(self.next_back())? {
                Value::Null => Ok(Value::Null),
                peeked => {
                    self.peeked_back = Some(peeked.clone());
                    Ok(peeked)
                }
            },
        }
    }

    fn next(&mut self) -> Option<Output> {
        self.peeked_front.take().map(Output::Value).or_else(|| {
            self.iter
                .next()
                .or_else(|| self.peeked_back.take().map(Output::Value))
        })
    }

    fn next_back(&mut self) -> Option<Output> {
        self.peeked_back.take().map(Output::Value).or_else(|| {
            self.iter
                .next_back()
                .or_else(|| self.peeked_front.take().map(Output::Value))
        })
    }
}

impl KotoType for Peekable {
    const TYPE: &'static str = "Peekable";
}

impl KotoObject for Peekable {
    fn object_type(&self) -> ValueString {
        PEEKABLE_TYPE_STRING.with(|t| t.clone())
    }

    fn copy(&self) -> Object {
        self.clone().into()
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        PEEKABLE_ENTRIES.with(|entries| entries.get(key).cloned())
    }

    fn is_iterable(&self) -> IsIterable {
        if self.iter.is_bidirectional() {
            IsIterable::BidirectionalIterator
        } else {
            IsIterable::ForwardIterator
        }
    }

    fn iterator_next(&mut self, _vm: &mut Vm) -> Option<Output> {
        self.peeked_front.take().map(Output::Value).or_else(|| {
            self.iter
                .next()
                .or_else(|| self.peeked_back.take().map(Output::Value))
        })
    }

    fn iterator_next_back(&mut self, _vm: &mut Vm) -> Option<Output> {
        self.peeked_back.take().map(Output::Value).or_else(|| {
            self.iter
                .next_back()
                .or_else(|| self.peeked_front.take().map(Output::Value))
        })
    }
}

fn peekable_entries() -> ValueMap {
    ObjectEntryBuilder::<Peekable>::new()
        .method("peek", |context| context.instance_mut()?.peek())
        .method("peek_back", |context| context.instance_mut()?.peek_back())
        .build()
}

thread_local! {
    static PEEKABLE_TYPE_STRING: ValueString = Peekable::TYPE.into();
    static PEEKABLE_ENTRIES: ValueMap = peekable_entries();
}

// For tests, see runtime/tests/iterator_tests.rs
