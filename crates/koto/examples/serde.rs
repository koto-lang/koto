use koto::{
    Result,
    prelude::*,
    serde::{from_koto_value, to_koto_value},
};
use serde::{Deserialize, Serialize};

fn main() -> Result<()> {
    let script = "
match request
  'one_to_four' then
    caption = 'one to four'
    numbers = 1, 2, 3, 4
  'five_to_eight' then 
    caption = 'five to eight'
    numbers = 5, 6, 7, 8

export {caption, numbers}
";

    let mut koto = Koto::default();

    // Add a 'request' value to the prelude
    koto.prelude()
        .insert("request", to_koto_value(Request::FiveToEight)?);
    koto.compile_and_run(script)?;

    // After running the script, deserialize the values that the script exported
    let exported: Exported = from_koto_value(koto.exports().clone())?;

    println!("Exported: '{}': {:?}", exported.caption, exported.numbers);

    Ok(())
}

#[derive(Deserialize, Serialize)]
struct Exported {
    caption: String,
    numbers: Vec<i64>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Request {
    OneToFour,
    FiveToEight,
}
