
fn main() {
    let script = r#"
        // Comment
        print("Hello, World!!!")
        print(42.0)
        a = 2
        b = a * 8 + 4
        print(b, 43.0, "Hiii")
    "#;

    match ks::parse(script) {
        Ok(ast) => {
            println!("{:?}\n", ast);
            let mut runtime = ks::Runtime::new();
            match runtime.run(&ast) {
                Ok(_) => {}
                Err(e) => println!("Error while running script:\n  {}", e),
            }
        }
        Err(e) => println!("Error while parsing source: {}", e),
    }
}
