use clap::Parser;

mod bench;
mod build;
mod demo;
mod examples;

#[derive(Parser)]
#[command(name = "xtask", about = "Development tasks for miros")]
enum Xtask {
    /// Build libmiros.so (release)
    Build,
    /// Build miros + compile the example programs against it
    Examples,
    /// Run a binary under miros (patches a copy's interpreter)
    Demo(demo::DemoArgs),
    /// Run benchmarks comparing miros against glibc
    Bench(bench::BenchArgs),
}

fn main() {
    match Xtask::parse() {
        Xtask::Build => {
            build::run();
        }
        Xtask::Examples => examples::run(),
        Xtask::Demo(args) => demo::run(args),
        Xtask::Bench(args) => bench::run(args),
    }
}
