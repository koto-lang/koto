use criterion::{criterion_group, criterion_main, Criterion};
use koto::{Ast, Parser, Runtime};
use std::{env::current_dir, fs::read_to_string};

struct BenchmarkRunner<'a> {
    ast: Ast,
    runtime: Runtime<'a>,
}

impl<'a> BenchmarkRunner<'a> {
    fn new(script_path: &str) -> Self {
        let mut path = current_dir().unwrap().canonicalize().unwrap();
        path.push("benches");
        path.push(script_path);
        let script = read_to_string(path).expect("Unable to load path");
        let ast = Parser::new()
            .parse(&script)
            .expect("Error while parsing script");

        Self {
            ast,
            runtime: Runtime::new(),
        }
    }

    fn run(&mut self) {
        self.runtime
            .run(&self.ast)
            .expect("Error while running script");
    }
}

pub fn koto_benchmark(c: &mut Criterion) {
    c.bench_function("fib10", |b| {
        let mut runner = BenchmarkRunner::new("fib10.koto");
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("vec4", |b| {
        let mut runner = BenchmarkRunner::new("vec4.koto");
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("enumerate", |b| {
        let mut runner = BenchmarkRunner::new("enumerate.koto");
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("spectral_norm", |b| {
        let mut runner = BenchmarkRunner::new("spectral_norm.koto");
        runner.runtime.set_args(&["4", "quiet"]);
        b.iter(|| {
            runner.run();
        })
    });
}

criterion_group!(benches, koto_benchmark);
criterion_main!(benches);
