# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Miros is an experimental ELF dynamic linker/loader written in Rust, intended to eventually replace `ld.so` on Linux systems. It uses `#![no_main]` with a naked assembly `_start` entry point and implements its own global allocator, syscall wrappers, and C ABI compatibility layer.

## Build Commands

```bash
cargo build          # Build the project
cargo test           # Run tests
cargo check          # Type-check without building
```

No custom build scripts, Makefile, or `.cargo/config.toml` exist.

## Architecture

### Boot Pipeline (Type-State Machine)

The linker uses phantom types to enforce a compile-time pipeline order:

```
_start (naked asm, src/start/mod.rs)
  → parse stack: argc, argv, env, auxv
  → Miros<Relocate> → .relocate() → Miros<AllocateTLS> → .allocate_tls() → Miros<InitArray> → .init_array()
  → Object<MapDependencies> → Object<AllocateTLS> → Object<GOTSetup>
```

`Miros` handles the linker itself (PIE self-relocation, TLS, init_array). `Object` handles loaded shared objects through a similar pipeline.

### Module Map

- **`src/start/`** — `_start` entry point, auxiliary vector parsing, environment variable iteration
- **`src/elf/`** — ELF format structs: headers, program headers, dynamic array, symbols, string tables, relocations, TLS
- **`src/objects/`** — Object lifecycle: `Miros` (self), `Object` (shared libs), `ObjectData` (metadata storage), pipeline strategies
- **`src/objects/strategies/`** — `Stratagem<T>` trait implementations: relocate, init_array, TLS allocation, dependency loading
- **`src/libc/`** — Syscall wrappers with C ABI: mmap/munmap, file I/O, environ, errno, threads
- **`src/syscall/x86_64/`** — Raw x86_64 syscall invocations via inline assembly
- **`src/global_allocator.rs`** — `GlobalAlloc` impl using mmap, initialized via init_array callback
- **`src/page_size.rs`** — Global page size state (from auxv)

### Key Patterns

- **Sealed traits** — `DynamicObject` trait uses `private::Sealed` to restrict implementations to `NonDynamic`/`Dynamic`
- **Bitfield types** — `bitbybit` crate for `ProtectionFlags`, `MapFlags`, `ProgramHeaderFlags`
- **`signature_matches_libc!`** — Compile-time macro checking that function signatures match libc's C ABI
- **`Stratagem<T>`** — Strategy trait for composable pipeline operations on object collections

## Important Constraints

- **No `.unwrap()`** — Causes runtime errors due to threading/allocator issues
- **x86_64 only** — Architecture-specific syscalls and assembly throughout
