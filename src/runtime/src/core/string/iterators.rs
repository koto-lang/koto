use crate::{
    value_iterator::{ExternalIterator2, ValueIterator, ValueIteratorOutput as Output},
    Value, ValueString,
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

/// An iterator that yields the lines contained in a string
///
/// - Lines end with either `\r\n` or `\n`.
/// - Line end characters aren't included in the resulting output.
/// - Empty lines are yielded as empty strings.
pub struct Lines {
    input: ValueString,
    start: usize,
}

impl Lines {
    pub fn new(input: ValueString) -> Self {
        Self { input, start: 0 }
    }
}

impl ExternalIterator2 for Lines {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            input: self.input.clone(),
            start: self.start,
        };
        ValueIterator::make_external_2(result)
    }
}

impl Iterator for Lines {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.start;
        if start < self.input.len() {
            let end = match self.input[start..].find('\n') {
                Some(end) => {
                    if end > start && self.input.as_bytes()[end - 1] == b'\r' {
                        start + end - 1
                    } else {
                        start + end
                    }
                }
                None => self.input.len(),
            };

            let result = Value::Str(self.input.with_bounds(start..end).unwrap());
            self.start = end + 1;
            Some(Output::Value(result))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining_bytes = self.input.len() - self.start;
        (1.min(remaining_bytes), Some(remaining_bytes))
    }
}
