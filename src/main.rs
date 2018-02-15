extern crate libc;
extern crate page_size;
extern crate syn;
#[cfg(target_os = "windows")]
extern crate winapi;

pub mod jit_memory;
pub mod compiler;
pub mod codegen;

use jit_memory::JitMemory;

fn main() {
    let test = include_str!("test.rs");
    let ast = syn::parse_file(test).expect("Unable to parse file");
    let buf = compiler::compile(ast).unwrap();
    let mut jit = JitMemory::from_assembly_buf(&buf).unwrap();

    let time_start = ::std::time::Instant::now();
    let sum = (jit.run())();
    let time_jit_execute = ::std::time::Instant::now();
    println!("the returned value is: {}", sum);
    println!("Execution time: {:?} ns", (time_jit_execute - time_start).subsec_nanos());
}