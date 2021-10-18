use crate::value_iterator::{ExternalIterator2, ValueIterator, ValueIteratorOutput};

/// An iterator that links the output of two iterators together in a chained sequence
pub struct Chain {
    iter_a: Option<ValueIterator>,
    iter_b: ValueIterator,
}

impl Chain {
    pub fn new(iter_a: ValueIterator, iter_b: ValueIterator) -> Self {
        Self {
            iter_a: Some(iter_a),
            iter_b,
        }
    }
}

impl ExternalIterator2 for Chain {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter_a: self.iter_a.as_ref().map(|iter| iter.make_copy()),
            iter_b: self.iter_b.make_copy(),
        };
        ValueIterator::make_external_2(result)
    }
}

impl Iterator for Chain {
    type Item = ValueIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter_a {
            Some(ref mut iter) => match iter.next() {
                output @ Some(_) => output,
                None => {
                    self.iter_a = None;
                    self.iter_b.next()
                }
            },
            None => self.iter_b.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.iter_a {
            Some(iter_a) => {
                let (lower_a, upper_a) = iter_a.size_hint();
                let (lower_b, upper_b) = self.iter_b.size_hint();

                let lower = lower_a.saturating_add(lower_b);
                let upper = match (upper_a, upper_b) {
                    (Some(a), Some(b)) => a.checked_add(b),
                    _ => None,
                };

                (lower, upper)
            }
            None => self.iter_b.size_hint(),
        }
    }
}

// See runtime/tests/iterator_adaptor_tests.rs for tests
