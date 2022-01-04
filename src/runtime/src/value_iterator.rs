use {
    crate::{
        IntRange, Num2, Num4, RuntimeError, Value, ValueList, ValueMap, ValueString, ValueTuple, Vm,
    },
    std::{cell::RefCell, fmt, ops::DerefMut, rc::Rc},
    unicode_segmentation::GraphemeCursor,
};

pub trait KotoIterator: Iterator<Item = ValueIteratorOutput> {
    /// Returns a copy of the iterator that (when possible), will produce the same output
    fn make_copy(&self) -> ValueIterator;

    /// Returns true when the iterator executes functions that may cause side effects
    ///
    /// This is used to determine whether or not the iterator is repeatable, which is used in
    /// iterator adaptors like chunks() or windows().
    fn might_have_side_effects(&self) -> bool;
}

#[derive(Clone, Debug)]
pub enum ValueIteratorOutput {
    Value(Value),
    ValuePair(Value, Value),
    Error(RuntimeError),
}

#[derive(Clone)]
pub struct ValueIterator(Rc<RefCell<dyn KotoIterator>>);

impl ValueIterator {
    pub fn make_external(external: impl KotoIterator + 'static) -> Self {
        Self(Rc::new(RefCell::new(external)))
    }

    pub fn with_range(range: IntRange) -> Self {
        Self::make_external(RangeIterator::new(range))
    }

    pub fn with_num2(n: Num2) -> Self {
        Self::make_external(Num2Iterator::new(n))
    }

    pub fn with_num4(n: Num4) -> Self {
        Self::make_external(Num4Iterator::new(n))
    }

    pub fn with_list(list: ValueList) -> Self {
        Self::make_external(ListIterator::new(list))
    }

    pub fn with_tuple(tuple: ValueTuple) -> Self {
        Self::make_external(TupleIterator::new(tuple))
    }

    pub fn with_map(map: ValueMap) -> Self {
        Self::make_external(MapIterator::new(map))
    }

    pub fn with_string(s: ValueString) -> Self {
        Self::make_external(StringIterator::new(s))
    }

    pub fn with_vm(vm: Vm) -> Self {
        Self::make_external(GeneratorIterator::new(vm))
    }

    pub fn make_copy(&self) -> Self {
        self.0.borrow().make_copy()
    }

    pub fn might_have_side_effects(&self) -> bool {
        self.0.borrow().might_have_side_effects()
    }

    // For internal functions that want to perform repeated iterations with a single borrow
    pub fn borrow_internals(
        &mut self,
        mut f: impl FnMut(&mut dyn KotoIterator) -> Option<ValueIteratorOutput>,
    ) -> Option<ValueIteratorOutput> {
        f(self.0.borrow_mut().deref_mut())
    }
}

impl Iterator for ValueIterator {
    type Item = ValueIteratorOutput;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.borrow_mut().next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.borrow().size_hint()
    }
}

impl fmt::Debug for ValueIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ValueIterator")
    }
}

// Convenience type alias for the rest of this module
type Output = ValueIteratorOutput;

#[derive(Clone)]
struct Num2Iterator {
    data: Num2,
    index: u8,
}

impl Num2Iterator {
    fn new(data: Num2) -> Self {
        Self { data, index: 0 }
    }
}

impl KotoIterator for Num2Iterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::make_external(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }
}

impl Iterator for Num2Iterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < 2 {
            let result = self.data[self.index as usize];
            self.index += 1;
            Some(Output::Value(Value::Number(result.into())))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = 2_u8.saturating_sub(self.index) as usize;
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct Num4Iterator {
    data: Num4,
    index: u8,
}

impl Num4Iterator {
    fn new(data: Num4) -> Self {
        Self { data, index: 0 }
    }
}

impl KotoIterator for Num4Iterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::make_external(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }
}

impl Iterator for Num4Iterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < 4 {
            let result = self.data[self.index as usize];
            self.index += 1;
            Some(Output::Value(Value::Number(result.into())))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = 4_u8.saturating_sub(self.index) as usize;
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct RangeIterator {
    data: IntRange,
    index: usize,
}

impl RangeIterator {
    fn new(data: IntRange) -> Self {
        Self { data, index: 0 }
    }
}

impl KotoIterator for RangeIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::make_external(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }
}

impl Iterator for RangeIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let range = &self.data;
        if range.is_ascending() {
            let result = range.start + self.index as isize;
            if result < range.end {
                self.index += 1;
                Some(ValueIteratorOutput::Value(Value::Number(result.into())))
            } else {
                None
            }
        } else {
            let result = range.start - self.index as isize;
            if result > range.end {
                self.index += 1;
                Some(ValueIteratorOutput::Value(Value::Number(result.into())))
            } else {
                None
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.data.len().saturating_sub(self.index) as usize;
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct ListIterator {
    data: ValueList,
    index: usize,
}

impl ListIterator {
    fn new(data: ValueList) -> Self {
        Self { data, index: 0 }
    }
}

impl KotoIterator for ListIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::make_external(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }
}

impl Iterator for ListIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self
            .data
            .data()
            .get(self.index)
            .map(|value| ValueIteratorOutput::Value(value.clone()));
        self.index += 1;
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.data.len().saturating_sub(self.index) as usize;
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct TupleIterator {
    data: ValueTuple,
    index: usize,
}

impl TupleIterator {
    fn new(data: ValueTuple) -> Self {
        Self { data, index: 0 }
    }
}

impl KotoIterator for TupleIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::make_external(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }
}

impl Iterator for TupleIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self
            .data
            .data()
            .get(self.index)
            .map(|value| ValueIteratorOutput::Value(value.clone()));
        self.index += 1;
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.data.data().len().saturating_sub(self.index) as usize;
        (remaining, Some(remaining))
    }
}

#[derive(Clone)]
struct MapIterator {
    data: ValueMap,
    index: usize,
}

impl MapIterator {
    fn new(data: ValueMap) -> Self {
        Self { data, index: 0 }
    }
}

impl KotoIterator for MapIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::make_external(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }
}

impl Iterator for MapIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let result =
            self.data.data().get_index(self.index).map(|(key, value)| {
                ValueIteratorOutput::ValuePair(key.value().clone(), value.clone())
            });
        self.index += 1;
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.data.data().len().saturating_sub(self.index) as usize;
        (remaining, Some(remaining))
    }
}

/// An iterator that yields the characters contained in the string
#[derive(Clone)]
pub struct StringIterator {
    data: ValueString,
    index: usize,
}

impl StringIterator {
    pub fn new(data: ValueString) -> Self {
        Self { data, index: 0 }
    }
}

impl KotoIterator for StringIterator {
    fn make_copy(&self) -> ValueIterator {
        ValueIterator::make_external(self.clone())
    }

    fn might_have_side_effects(&self) -> bool {
        false
    }
}

impl Iterator for StringIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = &self.data[self.index..];
        match GraphemeCursor::new(0, remaining.len(), true)
            .next_boundary(remaining, 0)
            .unwrap() // Safety: self.index will be on a grapheme boundary or at the string's end
        {
            Some(grapheme_end) => {
                let result = self.data
                    .with_bounds(self.index..self.index + grapheme_end)
                    .unwrap(); // Safety: Some(_) returned from next_boundary implies valid bounds
                self.index += grapheme_end;
                Some(ValueIteratorOutput::Value(Value::Str(result)))
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let upper_bound = self.data[self.index..].len();
        let lower_bound = if upper_bound == 0 { 0 } else { 1 };
        (lower_bound, Some(upper_bound))
    }
}

#[derive(Clone)]
pub struct GeneratorIterator {
    vm: Vm,
}

impl GeneratorIterator {
    pub fn new(vm: Vm) -> Self {
        Self { vm }
    }
}

impl KotoIterator for GeneratorIterator {
    fn make_copy(&self) -> ValueIterator {
        let new_vm = crate::vm::clone_generator_vm(&self.vm);
        ValueIterator::with_vm(new_vm)
    }

    fn might_have_side_effects(&self) -> bool {
        true
    }
}

impl Iterator for GeneratorIterator {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.vm.continue_running() {
            Ok(Value::Empty) => None,
            Ok(Value::TemporaryTuple(_)) => {
                unreachable!("Yield shouldn't produce temporary tuples")
            }
            Ok(result) => Some(ValueIteratorOutput::Value(result)),
            Err(error) => Some(ValueIteratorOutput::Error(error)),
        }
    }
}
