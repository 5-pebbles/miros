use clap::Parser;

mod bench;

#[derive(Parser)]
#[command(name = "xtask", about = "Development tasks for miros")]
enum Xtask {
    /// Run benchmarks comparing miros against glibc
    Bench(bench::BenchArgs),
}

fn main() {
    match Xtask::parse() {
        Xtask::Bench(args) => bench::run(args),
    }
}
