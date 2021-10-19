use crate::{
    value_iterator::{ExternalIterator2, ValueIterator, ValueIteratorOutput as Output},
    ValueString,
};

/// An iterator that outputs the individual bytes contained in a string
pub struct Bytes {
    input: ValueString,
    index: usize,
}

impl Bytes {
    pub fn new(input: ValueString) -> Self {
        Self { input, index: 0 }
    }
}

impl ExternalIterator2 for Bytes {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            input: self.input.clone(),
            index: self.index,
        };
        ValueIterator::make_external_2(result)
    }
}

impl Iterator for Bytes {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.input.as_bytes().get(self.index) {
            Some(byte) => {
                self.index += 1;
                Some(Output::Value(byte.into()))
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.input.len() - self.index;
        (remaining, Some(remaining))
    }
}
