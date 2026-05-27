use anyhow::{Result, bail};
use koto::prelude::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let script = "
from task import sleep, spawn

tasks = (1, 2, 3)
  .each |n|
    task.spawn ||
      # Wait for a few milliseconds
      await sleep n * 0.005
      n * n
  .to_tuple()

await task.join tasks
";

    let mut koto = Koto::default();
    let chunk = koto.compile(script)?;

    let result = koto.run_async(chunk).await?;

    match result {
        KValue::List(results) => {
            println!("Task results: {}", koto.value_to_string(results.into())?);
        }
        unexpected => bail!("Expected a List, found '{}'", unexpected.type_as_string()),
    }

    Ok(())
}
