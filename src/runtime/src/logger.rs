pub trait KotoLogger: Send + Sync {
    fn writeln(&self, output: &str);
}

pub struct DefaultLogger {}

impl KotoLogger for DefaultLogger {
    fn writeln(&self, output: &str) {
        println!("{}", output);
    }
}
