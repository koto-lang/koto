//! The core library for the Koto language

pub mod io;
pub mod iterator;
pub mod koto;
pub mod list;
pub mod map;
pub mod number;
pub mod os;
pub mod range;
pub mod string;
pub mod test;
pub mod tuple;
mod value_sort;

use crate::ValueMap;

#[derive(Clone)]
#[allow(missing_docs)]
pub struct CoreLib {
    pub io: ValueMap,
    pub iterator: ValueMap,
    pub koto: ValueMap,
    pub list: ValueMap,
    pub map: ValueMap,
    pub os: ValueMap,
    pub number: ValueMap,
    pub range: ValueMap,
    pub string: ValueMap,
    pub test: ValueMap,
    pub tuple: ValueMap,
}

impl Default for CoreLib {
    fn default() -> Self {
        Self {
            io: io::make_module(),
            iterator: iterator::make_module(),
            koto: koto::make_module(),
            list: list::make_module(),
            map: map::make_module(),
            os: os::make_module(),
            number: number::make_module(),
            range: range::make_module(),
            string: string::make_module(),
            test: test::make_module(),
            tuple: tuple::make_module(),
        }
    }
}
