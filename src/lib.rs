#![allow(unused_variables)]
#![allow(dead_code)]

extern crate libc;
extern crate page_size;
extern crate syn;
#[cfg(target_os = "windows")]
extern crate winapi;

mod jit_memory;
mod compiler;

pub use jit_memory::JitMemory;
pub use syn::parse_file;
pub use compiler::compile;