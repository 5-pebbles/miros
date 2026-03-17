#![feature(impl_trait_in_assoc_type)]
#![feature(c_variadic)]
#![feature(ptr_as_ref_unchecked)]
#![feature(type_changing_struct_update)]
#![feature(thread_id_value)]
#![feature(thread_local)]
#![feature(associated_type_defaults)]
#![feature(anonymous_lifetime_in_impl_trait)]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(dead_code)]
// SAFETY: Should prevent LLVM from recognizing patterns in our libc implementations.
// (e.g. strlen's byte-scanning loop) and replacing them with calls to those same functions.
// Avoiding infinite recursion → UB → ud2 in optimized builds.
#![cfg_attr(not(test), no_builtins)]
// NOTE: The entry point is defined in /src/start/mod.rs. o7
#![cfg_attr(not(test), no_main)]

mod global_allocator;

#[cfg_attr(target_arch = "x86_64", path = "syscall/x86_64/mod.rs")]
mod syscall;

mod elf;
mod error;
mod io_macros;
mod libc;
mod objects;
mod page_size;
mod start;
#[cfg(test)]
mod test_macros;
mod utils;
