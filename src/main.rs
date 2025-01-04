#![feature(impl_trait_in_assoc_type)]
#![feature(naked_functions)]
#![feature(ptr_as_ref_unchecked)]
#![feature(type_changing_struct_update)]
#![no_main]
#![allow(dead_code)]

#[cfg_attr(target_arch = "x86_64", path = "syscall/x86_64/mod.rs")]
mod syscall;

mod elf;
mod io_macros;
mod page_size;
mod start;
// mod shared_object;
mod static_pie;
mod utils;
