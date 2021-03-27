use {
    criterion::{criterion_group, criterion_main, Criterion},
    koto::Koto,
    std::{env::current_dir, fs::read_to_string},
};

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

struct BenchmarkRunner {
    runtime: Koto,
}

impl BenchmarkRunner {
    fn new(script_path: &str, args: &[String]) -> Self {
        let mut path = current_dir().unwrap().canonicalize().unwrap();
        path.push("..");
        path.push("..");
        path.push("koto");
        path.push("benches");
        path.push(script_path);
        let script = read_to_string(path).expect("Unable to load path");

        let mut runtime = Koto::new();
        match runtime.compile(&script) {
            Ok(_) => {
                runtime.settings.run_tests = true;

                if let Err(error) = runtime.run_with_args(&args) {
                    panic!("{}", error);
                }

                runtime.settings.run_tests = false;
            }
            Err(error) => panic!("{}", error),
        }

        Self { runtime }
    }

    fn run(&mut self) {
        if let Err(error) = self.runtime.run() {
            panic!("{}", error);
        }
    }
}

pub fn koto_benchmark(c: &mut Criterion) {
    c.bench_function("fib", |b| {
        let mut runner = BenchmarkRunner::new("fib_recursive.koto", &[]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("num4", |b| {
        let mut runner = BenchmarkRunner::new("num4.koto", &[]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("enumerate", |b| {
        let mut runner = BenchmarkRunner::new("enumerate.koto", &[]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("string_formatting", |b| {
        let mut runner = BenchmarkRunner::new(
            "string_formatting.koto",
            &["70".to_string(), "quiet".to_string()],
        );
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("spectral_norm", |b| {
        let mut runner = BenchmarkRunner::new(
            "spectral_norm.koto",
            &["2".to_string(), "quiet".to_string()],
        );
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("fannkuch", |b| {
        let mut runner =
            BenchmarkRunner::new("fannkuch.koto", &["4".to_string(), "quiet".to_string()]);
        b.iter(|| {
            runner.run();
        })
    });
    c.bench_function("n_body", |b| {
        let mut runner =
            BenchmarkRunner::new("n_body.koto", &["10".to_string(), "quiet".to_string()]);
        b.iter(|| {
            runner.run();
        })
    });
}

criterion_group!(benches, koto_benchmark);
criterion_main!(benches);
