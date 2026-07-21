#![feature(allocator_api)]
#![feature(c_variadic)]
#![feature(const_trait_impl)]
#![feature(const_cmp)]
#![feature(generic_atomic)]
#![feature(type_changing_struct_update)]
#![feature(thread_local)]
#![feature(stmt_expr_attributes)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(ptr_metadata)]
#![allow(dead_code)]
// The libm TT muncher recurses once per exported symbol (~120).
#![recursion_limit = "256"]
// Pervasively-unsafe crate (linker + libc + allocator) almost every line is the unsafe operation.
#![allow(unsafe_op_in_unsafe_fn)]
// SAFETY: Should prevent LLVM from recognizing patterns in our libc implementations.
// (e.g. strlen's byte-scanning loop) and replacing them with calls to those same functions.
// Avoiding infinite recursion → UB → ud2 in optimized builds.
#![cfg_attr(not(test), no_builtins)]
// NOTE: The entry point is defined in /src/start/mod.rs. o7
#![cfg_attr(not(test), no_main)]

mod allocator;
mod elf;
mod error;
mod io_macros;
mod libc;
mod objects;
mod page_size;
mod start;
mod syscall;
#[cfg(test)]
mod test_macros;
mod tls;
mod utils;
