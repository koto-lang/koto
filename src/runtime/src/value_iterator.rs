use {
    crate::{Error, Value, ValueList, ValueMap, Vm},
    std::sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntRange {
    pub start: isize,
    pub end: isize,
}

#[derive(Clone, Debug)]
pub enum Iterable {
    Range(IntRange),
    List(ValueList),
    Map(ValueMap),
    Generator(Arc<Mutex<Vm>>),
}

pub enum ValueIteratorOutput {
    Value(Value),
    ValuePair(Value, Value),
}

#[derive(Clone, Debug)]
pub struct ValueIterator {
    index: usize,
    iterable: Iterable,
}

impl ValueIterator {
    pub fn new(iterable: Iterable) -> Self {
        Self { index: 0, iterable }
    }

    pub fn with_range(range: IntRange) -> Self {
        Self::new(Iterable::Range(range))
    }

    pub fn with_list(list: ValueList) -> Self {
        Self::new(Iterable::List(list))
    }

    pub fn with_map(map: ValueMap) -> Self {
        Self::new(Iterable::Map(map))
    }

    pub fn with_vm(vm: Vm) -> Self {
        Self::new(Iterable::Generator(Arc::new(Mutex::new(vm))))
    }
}

impl Iterator for ValueIterator {
    type Item = Result<ValueIteratorOutput, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        use Value::Number;

        match &self.iterable {
            Iterable::Range(IntRange { start, end }) => {
                if start <= end {
                    // ascending range
                    let result = start + self.index as isize;
                    if result < *end {
                        self.index += 1;
                        Some(Ok(ValueIteratorOutput::Value(Number(result as f64))))
                    } else {
                        None
                    }
                } else {
                    // descending range
                    let result = start - self.index as isize - 1; // TODO avoid -1
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
            Iterable::Generator(vm) => {
                match vm.lock() {
                    Ok(mut vm_guard) => match vm_guard.continue_running() {
                        Ok(Value::Empty) => None,
                        Ok(Value::RegisterList(register_list)) => {
                            // TODO, instead of capturing values into a list here,
                            // return the VM and register list, and then the caller can copy
                            // the values into registers
                            Some(Ok(ValueIteratorOutput::Value(Value::List(
                                ValueList::from_slice(
                                    vm_guard
                                        .register_slice(register_list.start, register_list.count),
                                ),
                            ))))
                        }
                        Ok(result) => Some(Ok(ValueIteratorOutput::Value(result))),
                        Err(error) => Some(Err(error)),
                    },
                    Err(_) => Some(Err(Error::ErrorWithoutLocation {
                        message: "Failed to access generator VM".to_string(),
                    })),
                }
            }
        }
    }
}
