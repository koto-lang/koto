use {
    crate::{Error, Value, ValueList, ValueMap, ValueTuple, Vm},
    std::{
        fmt,
        sync::{Arc, Mutex},
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntRange {
    pub start: isize,
    pub end: isize,
}

pub enum ValueIteratorOutput {
    Value(Value),
    ValuePair(Value, Value),
}

pub type ValueIteratorResult = Result<ValueIteratorOutput, Error>;

#[derive(Debug)]
pub enum Iterable {
    Range(IntRange),
    List(ValueList),
    Tuple(ValueTuple),
    Map(ValueMap),
    Generator(Box<Vm>),
    External(ExternalIterator),
}

pub struct ExternalIterator(
    Box<dyn FnMut() -> Option<ValueIteratorResult> + Send + Sync + 'static>,
);

impl Iterator for ExternalIterator {
    type Item = ValueIteratorResult;

    fn next(&mut self) -> Option<Self::Item> {
        (self.0)()
    }
}

impl fmt::Debug for ExternalIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExternalIterator")
    }
}

#[derive(Debug)]
pub struct ValueIteratorInternals {
    index: usize,
    iterable: Iterable,
}

impl ValueIteratorInternals {
    fn new(iterable: Iterable) -> Self {
        Self { index: 0, iterable }
    }
}

impl Iterator for ValueIteratorInternals {
    type Item = ValueIteratorResult;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.iterable {
            Iterable::Range(IntRange { start, end }) => {
                use Value::Number;

                if start <= end {
                    // ascending range
                    let result = *start + self.index as isize;
                    if result < *end {
                        self.index += 1;
                        Some(Ok(ValueIteratorOutput::Value(Number(result as f64))))
                    } else {
                        None
                    }
                } else {
                    // descending range
                    let result = *start - self.index as isize - 1; // TODO avoid -1
                    if result >= *end {
                        self.index += 1;
                        Some(Ok(ValueIteratorOutput::Value(Number(result as f64))))
                    } else {
                        None
                    }
                }
            }
            Iterable::List(list) => {
                let result = list
                    .data()
                    .get(self.index)
                    .map(|value| Ok(ValueIteratorOutput::Value(value.clone())));
                self.index += 1;
                result
            }
            Iterable::Tuple(tuple) => {
                let result = tuple
                    .data()
                    .get(self.index)
                    .map(|value| Ok(ValueIteratorOutput::Value(value.clone())));
                self.index += 1;
                result
            }
            Iterable::Map(map) => {
                let result = match map.data().get_index(self.index) {
                    Some((key, value)) => Some(Ok(ValueIteratorOutput::ValuePair(
                        key.clone(),
                        value.clone(),
                    ))),
                    None => None,
                };

                self.index += 1;
                result
            }
            Iterable::Generator(vm) => match vm.continue_running() {
                Ok(Value::Empty) => None,
                Ok(Value::TemporaryTuple(_)) => {
                    unreachable!("Yield shouldn't produce temporary tuples")
                }
                Ok(result) => Some(Ok(ValueIteratorOutput::Value(result))),
                Err(error) => Some(Err(error)),
            },
            Iterable::External(external_iterator) => external_iterator.next(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ValueIterator(Arc<Mutex<ValueIteratorInternals>>);

impl ValueIterator {
    pub fn new(iterable: Iterable) -> Self {
        Self(Arc::new(Mutex::new(ValueIteratorInternals::new(iterable))))
    }

    pub fn with_range(range: IntRange) -> Self {
        Self::new(Iterable::Range(range))
    }

    pub fn with_list(list: ValueList) -> Self {
        Self::new(Iterable::List(list))
    }

    pub fn with_tuple(tuple: ValueTuple) -> Self {
        Self::new(Iterable::Tuple(tuple))
    }

    pub fn with_map(map: ValueMap) -> Self {
        Self::new(Iterable::Map(map))
    }

    pub fn with_vm(vm: Vm) -> Self {
        Self::new(Iterable::Generator(Box::new(vm)))
    }

    pub fn make_external(
        external: impl FnMut() -> Option<ValueIteratorResult> + Send + Sync + 'static,
    ) -> Self {
        Self::new(Iterable::External(ExternalIterator(Box::new(external))))
    }

    // For internal functions that want to perform repeated iterations with a single lock
    pub fn lock_internals(
        &mut self,
        mut f: impl FnMut(&mut ValueIteratorInternals) -> Option<ValueIteratorResult>,
    ) -> Option<ValueIteratorResult> {
        match self.0.lock() {
            Ok(mut internals) => f(&mut internals),
            Err(_) => Some(Err(Error::ErrorWithoutLocation {
                message: "Failed to access iterator internals".to_string(),
            })),
        }
    }
}

impl Iterator for ValueIterator {
    type Item = ValueIteratorResult;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.lock() {
            Ok(mut internals) => internals.next(),
            Err(_) => Some(Err(Error::ErrorWithoutLocation {
                message: "Failed to access iterator internals".to_string(),
            })),
        }
    }
}

pub fn is_iterable(value: &Value) -> bool {
    use Value::*;
    matches!(value, Range(_) | List(_) | Tuple(_) | Map(_) | Iterator(_))
}

pub fn make_iterator(value: &Value) -> Result<ValueIterator, ()> {
    use Value::*;
    let result = match value {
        Range(r) => ValueIterator::with_range(*r),
        List(l) => ValueIterator::with_list(l.clone()),
        Tuple(t) => ValueIterator::with_tuple(t.clone()),
        Map(m) => ValueIterator::with_map(m.clone()),
        Iterator(i) => i.clone(),
        _ => return Err(()),
    };
    Ok(result)
}
