use clap::Parser;

mod aliases;
mod bench;
mod build;
mod demo;
mod examples;
mod run;
mod test;

#[derive(Parser)]
#[command(name = "xtask", about = "Development tasks for miros")]
enum Xtask {
    /// Build libmiros.so (release)
    Build(build::BuildArgs),
    /// Regenerate the alias asm/version script from linked_aliases.def without building
    RegenerateAliases,
    /// Build miros + compile the example programs against it
    Examples,
    /// Run a binary under miros (patches a copy's interpreter)
    Demo(demo::DemoArgs),
    /// Run a binary under miros via direct invocation (`libmiros.so <binary>`)
    Run(run::RunArgs),
    /// Run benchmarks comparing miros against glibc
    Bench(bench::BenchArgs),
    /// Run the example e2e tests
    Test {
        /// Only run tests whose name contains this substring
        filter: Option<String>,
        /// Run each example through `libmiros.so <binary>` (direct invocation) instead of as its PT_INTERP
        #[arg(long)]
        direct: bool,
    },
}

fn main() {
    match Xtask::parse() {
        Xtask::Build(args) => {
            build::run(args);
        }
        Xtask::RegenerateAliases => {
            aliases::generate();
        }
        Xtask::Examples => {
            examples::run();
        }
        Xtask::Demo(args) => demo::run(args),
        Xtask::Run(args) => run::run(args),
        Xtask::Bench(args) => bench::run(args),
        Xtask::Test { filter, direct } => test::run(filter, direct),
    }
}
