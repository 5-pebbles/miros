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
// NOTE: The entry point is defined in /src/start/mod.rs. o7
#![no_main]
// SAFETY: Should prevent LLVM from recognizing patterns in our libc implementations.
// (e.g. strlen's byte-scanning loop) and replacing them with calls to those same functions.
// Avoiding infinite recursion → UB → ud2 in optimized builds.
#![no_builtins]

#[cfg_attr(target_arch = "x86_64", path = "syscall/x86_64/mod.rs")]
mod syscall;

mod elf;
mod io_macros;
mod libc;
mod objects;
mod page_size;
// mod shared_object;
mod error;
mod global_allocator;
mod start;
mod utils;
