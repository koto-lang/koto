#[macro_use]
extern crate pest_derive;

#[derive(Parser)]
#[grammar = "koto.pest"]
pub struct KotoParser;
