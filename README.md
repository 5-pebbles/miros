# Miros 🌸🌿

A monolithic runtime for x86_64 Linux, written in Rust. It runs unmodified glibc-linked binaries, intercepting symbol resolution and redirecting libc and pthreads calls to its own implementations. ELF loader, dynamic linker, C standard library, pthreads, libm, and memory allocator, fused into a single binary.

The split between `ld.so` and libc is a lie. The two depend on each other's internals: undocumented globals, unstable structs like `rtld_global_ro`, interleaved TLS and `pthread_cancel` coordination. They're versioned together, deployed together, and neither starts without the other. Why are they two separate binaries?

## Run a real program on it 🔧

`cargo xtask demo` patches the interpreter path in a copy of a binary and runs it under Miros:

```bash
cargo xtask build
cargo xtask demo /path/to/binary
```

For example, my website [auxv.org](https://auxv.org), a Rocket + tokio + rusqlite webserver, runs with no modifications:

```bash
cargo xtask demo ~/git/auxv-dot-org/target/release/auxv-dot-org \
    --features lenient-undefined-symbols --dir ~/git/auxv-dot-org -- \
    --http-only --http-port 8080
```

## Status

Nightly Rust, x86_64 only (for now). The C surface is incomplete: 300+ symbols and growing fast.

Run `cargo xtask --help` for the available commands.

### When a symbol is missing

The relocation pass collects every unresolved symbol and reports the full list:

```
Miros [Error]: Found Undefined Symbols [`foo`, `bar`]
```

Build with `--features lenient-undefined-symbols` to downgrade this to a warning. Unresolved symbols relocate to null, so a program only crashes if it actually calls one.

## Benchmarks 🏁

The benchmarks cover only the allocator. `cargo xtask bench` builds the same C harness against glibc's malloc and against Miros, pins with `taskset`, and reports trimmed medians.

```
[env] pinned to core 2
[compile] alloc_stress (glibc)
[compile] alloc_stress (miros)
[warmup] alloc_stress x 1
[run] alloc_stress - glibc x 10
[run] alloc_stress - miros x 10

=== alloc_stress ===
phase                           glibc        miros    glibc ns/op    miros ns/op     ratio
──────────────────────── ──────────── ──────────── ────────────── ────────────── ─────────
tight_32                      30.4 ms      46.3 ms           6.07           9.26    1.525x
mixed_1_to_2048               34.1 ms      19.0 ms          17.03           9.50    0.558x
realloc_32_to_8192            54.1 ms     126.2 ms          13.53          31.56    2.333x
large_256K                   237.3 ms     231.1 ms        4745.46        4622.74    0.974x
churn_shuffled                81.2 ms      28.7 ms          81.25          28.69    0.353x
calloc_mixed                 414.2 ms     220.6 ms         207.12         110.30    0.533x
TOTAL                        843.2 ms     670.8 ms   843201639.50   670837565.50    0.796x

--- alloc_stress (glibc) ---

 Performance counter stats for '/home/ghostbird/git/miros/benchmarks/bin/alloc_stress_glibc' (10 runs):

        3113300572      cycles:u                                                                ( +-  0.21% )
        2830189239      instructions:u                                                          ( +-  0.00% )
           2219634      cache-misses:u                                                          ( +- 17.84% )
             65659      dTLB-load-misses:u                                                      ( +-  1.53% )

       0.829983688 +- 0.003722800 seconds time elapsed  ( +-  0.45% )


--- alloc_stress (miros) ---

 Performance counter stats for '/home/ghostbird/git/miros/benchmarks/bin/alloc_stress_miros' (10 runs):

        2561611820      cycles:u                                                                ( +-  0.51% )
        2002906333      instructions:u                                                          ( +-  0.00% )
            311677      cache-misses:u                                                          ( +- 16.46% )
              1137      dTLB-load-misses:u                                                      ( +- 10.47% )

       0.710607881 +- 0.009150266 seconds time elapsed  ( +-  1.29% )
```

Ratios below 1.0 mean Miros is faster, above 1.0 mean glibc is faster.

The two losses are the price of security features. Randomized slot selection costs `tight_32`: glibc hands back the slot it just freed, Miros picks a random one. And binned size classes can't grow an allocation in place the way glibc's realloc does, so `realloc_32_to_8192` pays for a fresh allocation and a copy on every class crossing.

The allocator is the default, so I decided to make it do everything semi-well rather than win one benchmark: binned size classes, out-of-band metadata, and randomized slot selection within spans. Run with `--perf` to collect `perf stat` counters for the cache-level view.

## Blog series 📝

The build is documented at [auxv.org](https://auxv.org). These have drifted from the current code:

- [Frankenstein's Monster 🧟](https://auxv.org/projects/miros/frankensteins_monster) - what ELF files actually are and what a dynamic linker does with them
- [Where to `_start`?](https://auxv.org/projects/miros/where_to__start) - stack layout, the System V ABI, and bootstrapping from naked assembly into Rust
- [Slayer of Dragons, Eater of Bugs 🐔](https://auxv.org/projects/miros/slayer_of_dragons_eater_of_bugs) - debugging the runtime with `rust-lldb`, `readelf`, and a calculator

## Contributing 🤝

Contributions are welcome. Write idiomatic Rust (iterators, combinators, pattern matching), not C-in-Rust.

Code map:

- **`src/start/`** - `_start`, stack parsing, the `Bootstrap<Stage>` machine
- **`src/elf/`** - ELF format types
- **`src/objects/`** - `ObjectDataGraph`, symbol resolution, the `Stratagem` pipeline
- **`src/libc/`** - the C surface: stdio, fs, net, process, threads, math
- **`src/allocator/`** - the production allocator
- **`src/tls/`** - TLS layout, module registry, thread control blocks
- **`src/syscall/`** - raw syscalls in inline assembly

Check the [issues](https://github.com/5-pebbles/miros/issues) if you're looking for something to work on.

### LLM Usage

Use an LLM if you want. The contribution is yours, not the model's. It is not co-authored by Claude, and Claude does not learn from code review. PRs that show no attempt to understand the problem get closed.

Commit messages follow [COMMIT.md](COMMIT.md).
