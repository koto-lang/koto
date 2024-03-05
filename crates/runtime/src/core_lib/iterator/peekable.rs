//! A double-ended peekable iterator for Koto

use koto_derive::*;

use super::{iter_output_to_result, IteratorOutput};
use crate::{prelude::*, KIteratorOutput as Output, Result};

/// A double-ended peekable iterator for Koto
#[derive(Clone, KotoCopy, KotoType)]
pub struct Peekable {
    iter: KIterator,
    peeked_front: Option<KValue>,
    peeked_back: Option<KValue>,
}

#[koto_impl(runtime = crate)]
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
    pub fn make_value(iter: KIterator) -> KValue {
        KObject::from(Self::new(iter)).into()
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

    #[koto_method]
    fn peek(&mut self) -> Result<KValue> {
        let peeked = match self.peeked_front.clone() {
            Some(peeked) => peeked,
            None => match iter_output_to_result(self.next())? {
                None => return Ok(KValue::Null),
                Some(peeked) => {
                    self.peeked_front = Some(peeked.clone());
                    peeked
                }
            },
        };

        Ok(IteratorOutput::from(peeked).into())
    }

    #[koto_method]
    fn peek_back(&mut self) -> Result<KValue> {
        let peeked = match self.peeked_back.clone() {
            Some(peeked) => peeked,
            None => match iter_output_to_result(self.next_back())? {
                None => return Ok(KValue::Null),
                Some(peeked) => {
                    self.peeked_back = Some(peeked.clone());
                    peeked
                }
            },
        };

        Ok(IteratorOutput::from(peeked).into())
    }
}

impl KotoObject for Peekable {
    fn is_iterable(&self) -> IsIterable {
        if self.iter.is_bidirectional() {
            IsIterable::BidirectionalIterator
        } else {
            IsIterable::ForwardIterator
        }
    }

    fn iterator_next(&mut self, _vm: &mut KotoVm) -> Option<Output> {
        self.peeked_front.take().map(Output::Value).or_else(|| {
            self.iter
                .next()
                .or_else(|| self.peeked_back.take().map(Output::Value))
        })
    }

    fn iterator_next_back(&mut self, _vm: &mut KotoVm) -> Option<Output> {
        self.peeked_back.take().map(Output::Value).or_else(|| {
            self.iter
                .next_back()
                .or_else(|| self.peeked_front.take().map(Output::Value))
        })
    }
}

// For tests, see runtime/tests/iterator_tests.rs
