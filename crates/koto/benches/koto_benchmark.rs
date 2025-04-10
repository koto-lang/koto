use criterion::{Criterion, criterion_group, criterion_main};
use koto::{Ptr, prelude::*};
use std::{fs::read_to_string, path::PathBuf};

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

struct BenchmarkRunner {
    runtime: Koto,
    chunk: Ptr<Chunk>,
}

impl BenchmarkRunner {
    fn setup(script_path: &str, args: &[&str]) -> Self {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("..");
        path.push("..");
        path.push("koto");
        path.push("benches");
        path.push(script_path);
        let script = read_to_string(path).expect("Unable to load path");

        let mut runtime = Koto::new();
        let prelude = runtime.prelude();
        prelude.insert("geometry", koto_geometry::make_module());

        let chunk = match runtime.compile(&script) {
            Ok(chunk) => {
                runtime
                    .set_args(args.iter().map(|s| s.to_string()))
                    .unwrap();
                if let Err(error) = runtime.run(chunk.clone()) {
                    panic!("{error}");
                }
                chunk
            }
            Err(error) => panic!("{error}"),
        };

        // The benchmark tests will be run when first instantiated,
        // and can be skipped on subsequent runs
        runtime.set_run_tests(false);

        Self { runtime, chunk }
    }

    fn run(&mut self) {
        if let Err(error) = self.runtime.run(self.chunk.clone()) {
            panic!("{error}");
        }
    }
}

pub fn koto_benchmark(c: &mut Criterion) {
    c.bench_function("fib", |b| {
        let mut runner = BenchmarkRunner::setup("fib_recursive.koto", &[]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("enumerate", |b| {
        let mut runner = BenchmarkRunner::setup("enumerate.koto", &[]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("string_formatting", |b| {
        let mut runner = BenchmarkRunner::setup("string_formatting.koto", &["70", "quiet"]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("spectral_norm", |b| {
        let mut runner = BenchmarkRunner::setup("spectral_norm.koto", &["2", "quiet"]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("fannkuch", |b| {
        let mut runner = BenchmarkRunner::setup("fannkuch.koto", &["4", "quiet"]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("n_body", |b| {
        let mut runner = BenchmarkRunner::setup("n_body.koto", &["10", "quiet"]);
        b.iter(|| {
            runner.run();
        })
    });
}

criterion_group!(benches, koto_benchmark);
criterion_main!(benches);
