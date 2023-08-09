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

impl CoreLib {
    /// The core lib items made available in each Koto script
    pub fn prelude(&self) -> ValueMap {
        let result = ValueMap::default();
        result.add_map("io", self.io.clone());
        result.add_map("iterator", self.iterator.clone());
        result.add_map("koto", self.koto.clone());
        result.add_map("list", self.list.clone());
        result.add_map("map", self.map.clone());
        result.add_map("os", self.os.clone());
        result.add_map("number", self.number.clone());
        result.add_map("range", self.range.clone());
        result.add_map("string", self.string.clone());
        result.add_map("test", self.test.clone());
        result.add_map("tuple", self.tuple.clone());

        macro_rules! default_import {
            ($name:expr, $module:ident) => {{
                result.add_value($name, self.$module.data().get($name).unwrap().clone());
            }};
        }

        default_import!("assert", test);
        default_import!("assert_eq", test);
        default_import!("assert_ne", test);
        default_import!("assert_near", test);
        default_import!("print", io);
        default_import!("copy", koto);
        default_import!("deep_copy", koto);
        default_import!("type", koto);

        result
    }
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
