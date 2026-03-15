# Miros 🌸🌿

A from-scratch ELF dynamic linker/loader/C standard library/pthreads monolith written in Rust. I'm building it to understand (and eventually replace) `ld.so` on my systems.

Requires Rust nightly. Build with `cargo build`.

## What Can It Do? 🔧

Miros starts from a naked `_start` in assembly, self-relocates as a PIE, sets up its own TLS and allocator. From there it:

- **Load shared objects** — recursively resolves `.so` dependencies, sets up the GOT, and handles symbol resolution.
- **C standard library** — implements C standard library methods: `printf`, file I/O, `mmap`/`munmap`, etc.
  The C standard library isn't fully implemented, there are many symbols that will fail with `UndefinedSymbol` errors.
- **Symbol intercept** — overrides Glibc symbols and resolves them to Miros' own implementations.

## How It Works 🧠

The linker uses a type-state machine with phantom types to enforce a compile-time pipeline order:

```
_start (naked asm)
→ parse stack: argc, argv, env, auxv
→ Miros<Relocate> → Miros<AllocateTls> → Miros<InitArray>
→ Object<MapDependencies> → Object<AllocateTls> → Object<GOTSetup>
```

Each stage transition consumes the previous state. The `Stratagem<T>` trait provides composable pipeline operations for processing loaded objects.

The codebase breaks down into a few key areas:
- **`src/start/`** — the `_start` entry point, auxiliary vector parsing, and the `Miros` type-state machine
- **`src/elf/`** — ELF format parsing: headers, program headers, dynamic arrays, symbols, relocations, TLS
- **`src/objects/`** — object lifecycle management, dependency loading, and pipeline strategies
- **`src/libc/`** — syscall wrappers and C ABI implementations (`printf`, file I/O, memory, threads)
- **`src/syscall/x86_64/`** — raw syscall invocations via inline assembly

For the deep dive, check the blog series below.


## Blog Series 📝

I'm documenting the process of building miros at [auxv.org](https://auxv.org):

- [Frankenstein's Monster 🧟](https://auxv.org/projects/miros/frankensteins_monster) — what ELF files actually are and what a dynamic linker does with them
- [Where to `_start`?](https://auxv.org/projects/miros/where_to__start) — stack layout, the System V ABI, and bootstrapping from naked assembly into Rust
- [Slayer of Dragons, Eater of Bugs 🐔](https://auxv.org/projects/miros/slayer_of_dragons_eater_of_bugs) — debugging the runtime with `rust-lldb`, `readelf`, and a calculator.

## Contributing 🤝

Contributions are welcome! A few things to know:

- **Idiomatic Rust** — use iterators, combinators, pattern matching, and the type system. No C-in-Rust.
- **How to debug** — `cargo build && rust-lldb <program_linked_to_miros>` is the workflow. `readelf -r` for inspecting relocations.
- **Check for Supported Symbols** — the following fish command can be used to identify any `GLIBC` symbols Miros doesn't support within a given binary:

```fish
set BINARY ./examples/print_deadbeef
comm -23 (nm -D --undefined-only $BINARY | grep '@GLIBC' | awk '{print $NF}' | sed 's/@.*//' | sort | psub) (nm -D --defined-only ./target/debug/miros | awk '{print $NF}' | sort | psub)
```

Check the [issues](https://github.com/5-pebbles/miros/issues) if you're looking for something to work on (there's a lot).
