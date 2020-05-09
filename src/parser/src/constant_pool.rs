#![allow(dead_code)]

use std::{collections::HashMap, convert::TryInto};

#[derive(Clone, Default)]
pub struct ConstantPool {
    data: Vec<u8>,
    index: Vec<usize>,
    strings: HashMap<String, usize>,
    numbers: HashMap<[u8; 8], usize>,
}

impl ConstantPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
        self.index.shrink_to_fit();
        self.strings.clear();
        self.numbers.clear();
    }

    pub fn add_string(&mut self, s: &str) -> usize {
        match self.strings.get(s) {
            Some(index) => *index,
            None => {
                let data_position = self.data.len();
                let index = self.index.len();
                self.index.push(data_position);

                self.strings.insert(s.to_string(), index);

                let bytes = s.as_bytes();

                // short strings will do for now, TODO long string type (16bit max length? longer?)
                assert!(bytes.len() < 1 << 8);
                let len = bytes.len() as u8;

                self.data.push(len);
                self.data.extend_from_slice(bytes);

                index
            }
        }
    }

    pub fn get_string(&self, index: usize) -> &str {
        let data_position = self.index[index];
        let string_len = self.data[data_position] as usize;
        let start = data_position + 1;
        let end = start + string_len;
        std::str::from_utf8(&self.data[start..end]).expect("Invalid string data")
        // TODO Result
    }

    pub fn add_f64(&mut self, n: f64) -> usize {
        let bytes = n.to_ne_bytes();
        match self.numbers.get(&bytes) {
            Some(index) => *index,
            None => {
                let data_position = self.data.len();
                let index = self.index.len();
                self.index.push(data_position);

                self.numbers.insert(bytes, index);

                self.data.extend_from_slice(&bytes);

                index
            }
        }
    }

    pub fn get_f64(&self, index: usize) -> f64 {
        let start = self.index[index];
        let end = start + 8;
        f64::from_ne_bytes(self.data[start..end].try_into().unwrap()) // TODO Result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adding_strings() {
        let mut pool = ConstantPool::new();

        let s1 = "test";
        let s2 = "test2";

        // 1 byte for string length
        assert_eq!(0, pool.add_string(s1));
        assert_eq!(1, pool.add_string(s2));

        // don't duplicate strings
        assert_eq!(0, pool.add_string(s1));
        assert_eq!(1, pool.add_string(s2));

        assert_eq!(s1, pool.get_string(0));
        assert_eq!(s2, pool.get_string(1));

        assert_eq!(11, pool.data_len());
    }

    #[test]
    fn test_adding_numbers() {
        let mut pool = ConstantPool::new();

        let f1 = 1.23456789;
        let f2 = 9.87654321;

        assert_eq!(0, pool.add_f64(f1));
        assert_eq!(1, pool.add_f64(f2));

        // don't duplicate numbers
        assert_eq!(0, pool.add_f64(f1));
        assert_eq!(1, pool.add_f64(f2));

        assert_eq!(f1, pool.get_f64(0));
        assert_eq!(f2, pool.get_f64(1));

        assert_eq!(16, pool.data_len());
    }

    #[test]
    fn test_adding_mixed_types() {
        let mut pool = ConstantPool::new();

        let f1 = -1.1;
        let f2 = 99.9;
        let s1 = "O_o";
        let s2 = "^_^";

        assert_eq!(0, pool.add_f64(f1));
        assert_eq!(1, pool.add_string(s1));
        assert_eq!(2, pool.add_f64(f2));
        assert_eq!(3, pool.add_string(s2));

        assert_eq!(f1, pool.get_f64(0));
        assert_eq!(f2, pool.get_f64(2));
        assert_eq!(s1, pool.get_string(1));
        assert_eq!(s2, pool.get_string(3));

        assert_eq!(24, pool.data_len());
    }
}
