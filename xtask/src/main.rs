use clap::Parser;

mod aliases;
mod bench;
mod build;
mod demo;
mod examples;
mod test;

#[derive(Parser)]
#[command(name = "xtask", about = "Development tasks for miros")]
enum Xtask {
    /// Build libmiros.so (release)
    Build {
        /// Cargo features to pass through (comma- or space-separated), like `cargo --features`.
        #[arg(long)]
        features: Option<String>,
        /// CPU to target (e.g. `x86-64-v2`). Defaults to `native`.
        #[arg(long)]
        target_cpu: Option<String>,
    },
    /// Regenerate the alias asm/version script from linked_aliases.def without building
    RegenerateAliases,
    /// Build miros + compile the example programs against it
    Examples,
    /// Run a binary under miros (patches a copy's interpreter)
    Demo(demo::DemoArgs),
    /// Run benchmarks comparing miros against glibc
    Bench(bench::BenchArgs),
    /// Run the example e2e tests
    Test {
        /// Only run tests whose name contains this substring
        filter: Option<String>,
    },
}

fn main() {
    match Xtask::parse() {
        Xtask::Build {
            features,
            target_cpu,
        } => {
            build::run(features.as_deref(), target_cpu.as_deref());
        }
        Xtask::RegenerateAliases => {
            aliases::generate();
        }
        Xtask::Examples => examples::run(),
        Xtask::Demo(args) => demo::run(args),
        Xtask::Bench(args) => bench::run(args),
        Xtask::Test { filter } => test::run(filter),
    }
}
