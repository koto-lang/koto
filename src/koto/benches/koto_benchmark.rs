use criterion::{criterion_group, criterion_main, Criterion};
use koto::Koto;
use std::{env::current_dir, fs::read_to_string};

struct BenchmarkRunner<'a> {
    koto: Koto<'a>,
}

impl<'a> BenchmarkRunner<'a> {
    fn new(script_path: &str, args: Vec<String>) -> Self {
        let mut path = current_dir().unwrap().canonicalize().unwrap();
        path.push("benches");
        path.push(script_path);
        let script = read_to_string(path).expect("Unable to load path");

        let mut koto = Koto::new();
        if let Err(error) = koto.run_script_with_args(&script, args) {
            eprintln!("{}", error);
            assert!(false);
        }
        Self { koto }
    }

    fn run(&mut self) {
        if let Err(error) = self.koto.run() {
            eprintln!("{}", error);
            assert!(false);
        }
    }
}

pub fn koto_benchmark(c: &mut Criterion) {
    c.bench_function("fib10", |b| {
        let mut runner = BenchmarkRunner::new("fib10.koto", vec![]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("vec4", |b| {
        let mut runner = BenchmarkRunner::new("vec4.koto", vec![]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("enumerate", |b| {
        let mut runner = BenchmarkRunner::new("enumerate.koto", vec![]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("spectral_norm", |b| {
        let mut runner = BenchmarkRunner::new("spectral_norm.koto", vec!["4".to_string()]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("fannkuch", |b| {
        let mut runner =
            BenchmarkRunner::new("fannkuch.koto", vec!["4".to_string(), "quiet".to_string()]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("n_body", |b| {
        let mut runner =
            BenchmarkRunner::new("n_body.koto", vec!["1000".to_string(), "quiet".to_string()]);
        b.iter(|| {
            runner.run();
        })
    });
}

criterion_group!(benches, koto_benchmark);
criterion_main!(benches);
