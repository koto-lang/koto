pub mod iterator;
pub mod list;
pub mod map;
pub mod number;
pub mod range;
pub mod string;
pub mod tuple;

use crate::ValueMap;

#[derive(Clone)]
pub struct CoreLib {
    pub iterator: ValueMap,
    pub list: ValueMap,
    pub map: ValueMap,
    pub number: ValueMap,
    pub range: ValueMap,
    pub string: ValueMap,
    pub tuple: ValueMap,
}

impl Default for CoreLib {
    fn default() -> Self {
        Self {
            iterator: iterator::make_module(),
            list: list::make_module(),
            map: map::make_module(),
            number: number::make_module(),
            range: range::make_module(),
            string: string::make_module(),
            tuple: tuple::make_module(),
        }
    }
}

