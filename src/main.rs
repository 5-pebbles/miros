#![feature(impl_trait_in_assoc_type)]
#![feature(c_variadic)]
#![feature(ptr_as_ref_unchecked)]
#![feature(type_changing_struct_update)]
#![no_main]
#![allow(dead_code)]

#[cfg_attr(target_arch = "x86_64", path = "syscall/x86_64/mod.rs")]
mod syscall;

mod elf;
mod io_macros;
mod libc;
// mod linking;
mod page_size;
// mod shared_object;
mod global_allocator;
mod start;
mod static_pie;
mod utils;
