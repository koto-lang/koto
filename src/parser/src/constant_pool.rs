#![allow(dead_code)]

use std::{collections::HashMap, convert::TryInto};

#[derive(Default)]
pub struct ConstantPool {
    data: Vec<u8>,
    strings: HashMap<String, usize>,
    numbers: HashMap<[u8; 8], usize>,
}

impl ConstantPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.data.len() as usize
    }

    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
        self.strings.clear();
        self.numbers.clear();
    }

    pub fn add_string(&mut self, s: &str) -> usize {
        match self.strings.get(s) {
            Some(index) => *index,
            None => {
                let index = self.len();
                self.strings.insert(s.to_string(), index);

                let bytes = s.as_bytes();

                // short strings will do for now, TODO long string type (16bit max length? longer?)
                assert!(bytes.len() < 1 << 8);
                let len = bytes.len() as u8;

                self.data.extend_from_slice(&[len]);
                self.data.extend_from_slice(bytes);

                index
            }
        }
    }

    pub fn get_string(&self, index: usize) -> &str {
        let string_len =self.data[index] as usize;
        std::str::from_utf8(&self.data[(index + 1)..(index + 1 + string_len)])
            .expect("Invalid string data")
    }

    pub fn add_f64(&mut self, n: f64) -> usize {
        let bytes = n.to_ne_bytes();
        match self.numbers.get(&bytes) {
            Some(index) => *index,
            None => {
                let index = self.len();
                self.numbers.insert(bytes, index);

                self.data.extend_from_slice(&bytes);

                index
            }
        }
    }

    pub fn get_f64(&self, index: usize) -> f64 {
        f64::from_ne_bytes(self.data[index..index + 8].try_into().unwrap())
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
        assert_eq!(5, pool.add_string(s2));

        // don't duplicate strings
        assert_eq!(0, pool.add_string(s1));
        assert_eq!(5, pool.add_string(s2));

        assert_eq!(s1, pool.get_string(0));
        assert_eq!(s2, pool.get_string(5));
        assert_eq!(11, pool.len());
    }

    #[test]
    fn test_adding_numbers() {
        let mut pool = ConstantPool::new();

        let f1 = 1.23456789;
        let f2 = 9.87654321;

        assert_eq!(0, pool.add_f64(f1));
        assert_eq!(8, pool.add_f64(f2));

        // don't duplicate numbers
        assert_eq!(0, pool.add_f64(f1));
        assert_eq!(8, pool.add_f64(f2));

        assert_eq!(f1, pool.get_f64(0));
        assert_eq!(f2, pool.get_f64(8));
        assert_eq!(16, pool.len());
    }

    #[test]
    fn test_adding_mixed_types() {
        let mut pool = ConstantPool::new();

        let f1 = -1.1;
        let f2 = 99.9;
        let s1 = "O_o";
        let s2 = "^_^";

        assert_eq!(0, pool.add_f64(f1));
        assert_eq!(8, pool.add_string(s1));
        assert_eq!(12, pool.add_f64(f2));
        assert_eq!(20, pool.add_string(s2));

        assert_eq!(f1, pool.get_f64(0));
        assert_eq!(f2, pool.get_f64(12));
        assert_eq!(s1, pool.get_string(8));
        assert_eq!(s2, pool.get_string(20));

        assert_eq!(24, pool.len());
    }
}
