use {
    super::collect_pair,
    crate::{
        make_runtime_error,
        value_iterator::{ExternalIterator, ValueIterator, ValueIteratorOutput as Output},
        CallArgs, Value, Vm,
    },
};

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

impl ExternalIterator for Chain {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter_a: self.iter_a.as_ref().map(|iter| iter.make_copy()),
            iter_b: self.iter_b.make_copy(),
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for Chain {
    type Item = Output;

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

/// An iterator that runs a function on each output value from the adapted iterator
pub struct Each {
    iter: ValueIterator,
    function: Value,
    vm: Vm,
}

impl Each {
    pub fn new(iter: ValueIterator, function: Value, vm: Vm) -> Self {
        Self { iter, function, vm }
    }
}

impl ExternalIterator for Each {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            function: self.function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for Each {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|output| {
            let function = self.function.clone();
            let functor_result = match output {
                Output::Value(value) => self.vm.run_function(function, CallArgs::Single(value)),
                Output::ValuePair(a, b) => {
                    self.vm.run_function(function, CallArgs::AsTuple(&[a, b]))
                }
                other => return other,
            };
            match functor_result {
                Ok(result) => Output::Value(result),
                Err(error) => Output::Error(error),
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An iterator that cycles through the adapted iterator infinitely
pub struct Cycle {
    stored: ValueIterator,
    operated: ValueIterator,
}

impl Cycle {
    pub fn new(iterator: ValueIterator) -> Self {
        Self {
            stored: iterator.make_copy(),
            operated: iterator,
        }
    }
}

impl ExternalIterator for Cycle {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            stored: self.stored.make_copy(),
            operated: self.operated.make_copy(),
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for Cycle {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.operated.next() {
            None => {
                self.operated = self.stored.make_copy();
                self.operated.next()
            }
            other => other,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.stored.size_hint() {
            // If the incoming iterator is empty, this iterator is empty
            (0, Some(0)) => (0, Some(0)),
            // Even if we know the size hint of the incoming iterator we can not know
            // the upper bound of this iterator since it is infinite
            (0, _) => (0, None),
            // An infinite iterator has no upper bound
            // and the maximum possible lower bound
            // https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.size_hint
            _ => (usize::MAX, None),
        }
    }
}

/// An iterator that attaches an enumerated iteration position to each value
pub struct Enumerate {
    iter: ValueIterator,
    index: usize,
}

impl Enumerate {
    pub fn new(iter: ValueIterator) -> Self {
        Self { iter, index: 0 }
    }
}

impl ExternalIterator for Enumerate {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            index: self.index,
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for Enumerate {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self
            .iter
            .next()
            .map(collect_pair) // Collect pairs for the RHS of the enumeration
            .map(|output| match output {
                // The output can be a ValuePair
                Output::Value(value) => Output::ValuePair(self.index.into(), value),
                other => other,
            });
        self.index += 1;
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An iterator that inserts a separator value between each output value from the adapted iterator
pub struct Intersperse {
    iter: ValueIterator,
    peeked: Option<Output>,
    next_is_separator: bool,
    separator: Value,
}

impl Intersperse {
    pub fn new(iter: ValueIterator, separator: Value) -> Self {
        Self {
            iter,
            peeked: None,
            next_is_separator: false,
            separator,
        }
    }
}

impl ExternalIterator for Intersperse {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            peeked: self.peeked.clone(),
            next_is_separator: self.next_is_separator,
            separator: self.separator.clone(),
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for Intersperse {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.peeked.take().or_else(|| self.iter.next());

        match next {
            output @ Some(_) => {
                let result = if self.next_is_separator {
                    self.peeked = output;
                    Some(Output::Value(self.separator.clone()))
                } else {
                    output
                };

                self.next_is_separator = !self.next_is_separator;
                result
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        intersperse_size_hint(&self.iter, self.next_is_separator)
    }
}

/// An iterator that inserts a separator value between each output value from the adapted iterator
///
/// The separator value is the result of calling a provided separator function.
pub struct IntersperseWith {
    iter: ValueIterator,
    peeked: Option<Output>,
    next_is_separator: bool,
    separator_function: Value,
    vm: Vm,
}

impl IntersperseWith {
    pub fn new(iter: ValueIterator, separator_function: Value, vm: Vm) -> Self {
        Self {
            iter,
            peeked: None,
            next_is_separator: false,
            separator_function,
            vm,
        }
    }
}

impl ExternalIterator for IntersperseWith {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            peeked: self.peeked.clone(),
            next_is_separator: self.next_is_separator,
            separator_function: self.separator_function.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for IntersperseWith {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.peeked.take().or_else(|| self.iter.next());

        match next {
            output @ Some(_) => {
                let result = if self.next_is_separator {
                    self.peeked = output;
                    Some(
                        match self
                            .vm
                            .run_function(self.separator_function.clone(), CallArgs::None)
                        {
                            Ok(result) => Output::Value(result),
                            Err(error) => Output::Error(error),
                        },
                    )
                } else {
                    output
                };

                self.next_is_separator = !self.next_is_separator;
                result
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        intersperse_size_hint(&self.iter, self.next_is_separator)
    }
}

fn intersperse_size_hint(iter: &ValueIterator, next_is_separator: bool) -> (usize, Option<usize>) {
    let (lower, upper) = iter.size_hint();
    let offset = !next_is_separator as usize;

    let lower = lower.saturating_sub(offset).saturating_add(lower);
    let upper = upper.and_then(|upper| upper.saturating_sub(offset).checked_add(upper));

    (lower, upper)
}

/// An iterator that skips over values that fail a predicate, and keeps those that pass
pub struct Keep {
    iter: ValueIterator,
    predicate: Value,
    vm: Vm,
}

impl Keep {
    pub fn new(iter: ValueIterator, predicate: Value, vm: Vm) -> Self {
        Self {
            iter,
            predicate,
            vm,
        }
    }
}

impl ExternalIterator for Keep {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            predicate: self.predicate.clone(),
            vm: self.vm.spawn_shared_vm(),
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for Keep {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        for output in &mut self.iter {
            let predicate = self.predicate.clone();
            let predicate_result = match &output {
                Output::Value(value) => self
                    .vm
                    .run_function(predicate, CallArgs::Single(value.clone())),
                Output::ValuePair(a, b) => self
                    .vm
                    .run_function(predicate, CallArgs::AsTuple(&[a.clone(), b.clone()])),
                error @ Output::Error(_) => return Some(error.clone()),
            };

            let result = match predicate_result {
                Ok(Value::Bool(false)) => continue,
                Ok(Value::Bool(true)) => output,
                Ok(unexpected) => Output::Error(make_runtime_error!(format!(
                    "iterator.keep: Expected a Bool to be returned from the \
                             predicate, found '{}'",
                    unexpected.type_as_string(),
                ))),
                Err(error) => Output::Error(error),
            };

            return Some(result);
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_lower, upper) = self.iter.size_hint();
        (0, upper)
    }
}

/// An iterator that takes up to N values from the adapted iterator, and then stops
pub struct Take {
    iter: ValueIterator,
    remaining: usize,
}

impl Take {
    pub fn new(iter: ValueIterator, count: usize) -> Self {
        Self {
            iter,
            remaining: count,
        }
    }
}

impl ExternalIterator for Take {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
            remaining: self.remaining,
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for Take {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.remaining -= 1;
            self.iter.next()
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        (
            lower.min(self.remaining),
            upper.map(|upper| upper.min(self.remaining)),
        )
    }
}

/// An iterator that combines the output of two iterators, 'zipping' output pairs together
pub struct Zip {
    iter_a: ValueIterator,
    iter_b: ValueIterator,
}

impl Zip {
    pub fn new(iter_a: ValueIterator, iter_b: ValueIterator) -> Self {
        Self { iter_a, iter_b }
    }
}

impl ExternalIterator for Zip {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter_a: self.iter_a.make_copy(),
            iter_b: self.iter_b.make_copy(),
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for Zip {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter_a.next().map(collect_pair) {
            Some(Output::Value(value_a)) => match self.iter_b.next().map(collect_pair) {
                Some(Output::Value(value_b)) => Some(Output::ValuePair(value_a, value_b)),
                error @ Some(Output::Error(_)) => error,
                _ => None,
            },
            error @ Some(Output::Error(_)) => error,
            _ => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower_a, upper_a) = self.iter_a.size_hint();
        let (lower_b, upper_b) = self.iter_b.size_hint();

        let lower = lower_a.min(lower_b);
        let upper = match (upper_a, upper_b) {
            (Some(upper_a), Some(upper_b)) => Some(upper_a.min(upper_b)),
            _ => None,
        };

        (lower, upper)
    }
}

/// An iterator that outputs the first element from any ValuePairs
pub struct PairFirst {
    iter: ValueIterator,
}

impl PairFirst {
    pub fn new(iter: ValueIterator) -> Self {
        Self { iter }
    }
}

impl ExternalIterator for PairFirst {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for PairFirst {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Output::ValuePair(first, _)) => Some(Output::Value(first)),
            other => other,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An iterator that outputs the second element from any ValuePairs
pub struct PairSecond {
    iter: ValueIterator,
}

impl PairSecond {
    pub fn new(iter: ValueIterator) -> Self {
        Self { iter }
    }
}

impl ExternalIterator for PairSecond {
    fn make_copy(&self) -> ValueIterator {
        let result = Self {
            iter: self.iter.make_copy(),
        };
        ValueIterator::make_external(result)
    }
}

impl Iterator for PairSecond {
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Output::ValuePair(_, second)) => Some(Output::Value(second)),
            other => other,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

// See runtime/tests/iterator_adaptor_tests.rs for tests
