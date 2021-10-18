use {
    crate::{
        Mutex, Num2, Num4, RuntimeError, Value, ValueList, ValueMap, ValueString, ValueTuple, Vm,
    },
    std::{fmt, sync::Arc},
    unicode_segmentation::GraphemeCursor,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntRange {
    pub start: isize,
    pub end: isize,
}

impl IntRange {
    pub fn is_ascending(&self) -> bool {
        self.start <= self.end
    }

    fn len(&self) -> usize {
        if self.is_ascending() {
            (self.end - self.start) as usize
        } else {
            (self.start - self.end) as usize
        }
    }
}

#[derive(Clone)]
pub enum ValueIteratorOutput {
    Value(Value),
    ValuePair(Value, Value),
    Error(RuntimeError),
}

#[derive(Clone)]
pub enum Iterable {
    Num2(Num2),
    Num4(Num4),
    Range(IntRange),
    List(ValueList),
    Tuple(ValueTuple),
    Map(ValueMap),
    Str(ValueString),
    Generator(Arc<Mutex<Vm>>),
    External(ExternalIterator),
    External2(Arc<Mutex<dyn ExternalIterator2>>),
}

#[derive(Clone)]
pub struct ExternalIterator(
    Arc<Mutex<dyn FnMut() -> Option<ValueIteratorOutput> + Send + Sync + 'static>>,
);

impl Iterator for ExternalIterator {
    type Item = ValueIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        (self.0.lock())()
    }
}

pub trait ExternalIterator2: Iterator<Item = ValueIteratorOutput> + Send + Sync {
    /// Returns a copy of the iterator that (when possible), will produce the same output
    fn make_copy(&self) -> ValueIterator;
}

#[derive(Clone)]
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
    type Item = ValueIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        use Value::Number;

        match &mut self.iterable {
            Iterable::Num2(n) => {
                if self.index < 2 {
                    let result = ValueIteratorOutput::Value(Number(n[self.index].into()));
                    self.index += 1;
                    Some(result)
                } else {
                    None
                }
            }
            Iterable::Num4(n) => {
                if self.index < 4 {
                    let result = ValueIteratorOutput::Value(Number(n[self.index].into()));
                    self.index += 1;
                    Some(result)
                } else {
                    None
                }
            }
            Iterable::Range(range @ IntRange { .. }) => {
                if range.is_ascending() {
                    let result = range.start + self.index as isize;
                    if result < range.end {
                        self.index += 1;
                        Some(ValueIteratorOutput::Value(Number(result.into())))
                    } else {
                        None
                    }
                } else {
                    let result = range.start - self.index as isize;
                    if result > range.end {
                        self.index += 1;
                        Some(ValueIteratorOutput::Value(Number(result.into())))
                    } else {
                        None
                    }
                }
            }
            Iterable::List(list) => {
                let result = list
                    .data()
                    .get(self.index)
                    .map(|value| ValueIteratorOutput::Value(value.clone()));
                self.index += 1;
                result
            }
            Iterable::Tuple(tuple) => {
                let result = tuple
                    .data()
                    .get(self.index)
                    .map(|value| ValueIteratorOutput::Value(value.clone()));
                self.index += 1;
                result
            }
            Iterable::Map(map) => {
                let result = map.data().get_index(self.index).map(|(key, value)| {
                    ValueIteratorOutput::ValuePair(key.value().clone(), value.clone())
                });
                self.index += 1;
                result
            }
            Iterable::Str(s) => {
                let remaining = &s[self.index..];
                match GraphemeCursor::new(0, remaining.len(), true)
                    .next_boundary(remaining, 0)
                    .unwrap() // complete chunk is provided to next_boundary
                {
                    Some(grapheme_end) => {
                        let result = s
                            .with_bounds(self.index..self.index + grapheme_end)
                            .unwrap(); // Some returned from next_boundary implies valid bounds
                        self.index += grapheme_end;
                        Some(ValueIteratorOutput::Value(Value::Str(result)))
                    }
                    None => None,
                }
            }
            Iterable::Generator(vm) => match vm.lock().continue_running() {
                Ok(Value::Empty) => None,
                Ok(Value::TemporaryTuple(_)) => {
                    unreachable!("Yield shouldn't produce temporary tuples")
                }
                Ok(result) => Some(ValueIteratorOutput::Value(result)),
                Err(error) => Some(ValueIteratorOutput::Error(error)),
            },
            Iterable::External(external_iterator) => external_iterator.next(),
            Iterable::External2(external_iterator) => external_iterator.lock().next(),
        }
    }
}

#[derive(Clone)]
pub struct ValueIterator(Arc<Mutex<ValueIteratorInternals>>);

impl ValueIterator {
    pub fn new(iterable: Iterable) -> Self {
        Self(Arc::new(Mutex::new(ValueIteratorInternals::new(iterable))))
    }

    pub fn with_range(range: IntRange) -> Self {
        Self::new(Iterable::Range(range))
    }

    pub fn with_num2(n: Num2) -> Self {
        Self::new(Iterable::Num2(n))
    }

    pub fn with_num4(n: Num4) -> Self {
        Self::new(Iterable::Num4(n))
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

    pub fn with_string(s: ValueString) -> Self {
        Self::new(Iterable::Str(s))
    }

    pub fn with_vm(vm: Vm) -> Self {
        Self::new(Iterable::Generator(Arc::new(Mutex::new(vm))))
    }

    pub fn make_copy(&self) -> Self {
        let internals = self.0.lock();
        match &internals.iterable {
            Iterable::External2(external) => external.lock().make_copy(),
            _ => Self(Arc::new(Mutex::new(internals.clone()))),
        }
    }

    pub fn make_external(
        external: impl FnMut() -> Option<ValueIteratorOutput> + Send + Sync + 'static,
    ) -> Self {
        Self::new(Iterable::External(ExternalIterator(Arc::new(Mutex::new(
            external,
        )))))
    }

    pub fn make_external_2(external: impl ExternalIterator2 + 'static) -> Self {
        Self::new(Iterable::External2(Arc::new(Mutex::new(external))))
    }

    // For internal functions that want to perform repeated iterations with a single lock
    pub fn lock_internals(
        &mut self,
        mut f: impl FnMut(&mut ValueIteratorInternals) -> Option<ValueIteratorOutput>,
    ) -> Option<ValueIteratorOutput> {
        f(&mut self.0.lock())
    }
}

impl Iterator for ValueIterator {
    type Item = ValueIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.lock().next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        use Iterable::*;

        let internals = self.0.lock();
        let index = internals.index;

        let iterable_size = match &internals.iterable {
            Num2(_) => 2,
            Num4(_) => 4,
            Range(r) => r.len(),
            List(l) => l.len(),
            Tuple(t) => t.data().len(),
            Map(m) => m.len(),
            Str(s) => {
                let upper_bound = s[index..].len();
                let lower_bound = if upper_bound == 0 { 0 } else { 1 };
                return (lower_bound, Some(upper_bound));
            }
            Generator(_) => 0,
            External(_) => 0, // TODO
            External2(external_iterator) => return external_iterator.lock().size_hint(),
        };

        let remaining = iterable_size - index;

        (remaining, Some(remaining))
    }
}

impl fmt::Debug for ValueIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ValueIterator")
    }
}

pub fn make_iterator(value: &Value) -> Result<ValueIterator, ()> {
    use Value::*;
    let result = match value {
        Range(r) => ValueIterator::with_range(*r),
        Num2(n) => ValueIterator::with_num2(*n),
        Num4(n) => ValueIterator::with_num4(*n),
        List(l) => ValueIterator::with_list(l.clone()),
        Tuple(t) => ValueIterator::with_tuple(t.clone()),
        Map(m) => ValueIterator::with_map(m.clone()),
        Str(s) => ValueIterator::with_string(s.clone()),
        Iterator(i) => i.clone(),
        _ => return Err(()),
    };
    Ok(result)
}
