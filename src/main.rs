#![allow(unused_imports)]
#![feature(rustc_private)]
extern crate rustc;
extern crate rustc_driver;
extern crate rustc_trans_utils;
extern crate syntax;

use syntax::feature_gate::UnstableFeatures;
use rustc_driver::{driver, Compilation, CompilerCalls, RustcDefaultCalls};
use rustc_trans_utils::trans_crate::TransCrate;
use rustc::session::{config, Session};
use rustc::session::config::{ErrorOutputType, Input};

extern crate libc;
extern crate page_size;
extern crate syn;
#[cfg(target_os = "windows")]
extern crate winapi;

pub mod jit_memory;
pub mod compiler;
pub mod codegen;

use jit_memory::JitMemory;
use compiler::AssemblyBuf;

fn main() {
    // start the parsing session

    let output_type = rustc::session::config::OutputType::Bitcode;
    let test = include_str!("test.rs");
    let ast = syn::parse_file(test).expect("Unable to parse file");
    
    println!("{:#?}", ast);

    let cpuinfo_buf = AssemblyBuf {
        instructions: vec![
            0x55,                     // push   rbp
            0x48, 0x89, 0xE5,         // mov    rbp,rsp
            0xB0, 0x04,               // mov    al,0x4
            0x5D,                     // pop    rbp
            0xC3,                     // ret
        ],
    };
    let mut jit = JitMemory::new(1).unwrap();
    jit.load_assembly(&cpuinfo_buf).unwrap();

    let time_start = ::std::time::Instant::now();
    let sum = (jit.get_entry_point_fn())();
    let time_jit_execute = ::std::time::Instant::now();
    println!("the returned value is: {}", sum);
    println!("Execution time: {:?} ns", (time_jit_execute - time_start).subsec_nanos());
}
