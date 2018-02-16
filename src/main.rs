#![allow(unused_variables)]

extern crate libc;
extern crate page_size;
extern crate syn;
#[cfg(target_os = "windows")]
extern crate winapi;
extern crate notify;

pub mod jit_memory;
pub mod compiler;
pub mod codegen;

use jit_memory::JitMemory;

use notify::{Watcher, RecursiveMode, watcher};
use std::sync::mpsc::channel;
use std::time::Duration;
use std::fs::File;
use std::io::Read;
use notify::DebouncedEvent;

const FILE_PATH: &str = "~/Development/gsr-jit/src/test.rs";

fn main() {

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_millis(200)).unwrap();

    watcher.watch("~/Development/gsr-jit/src/test.rs", RecursiveMode::Recursive).unwrap();

    let mut jit = None;
    let mut compile_duration = None;

    do_jit(&mut jit, &mut compile_duration, FILE_PATH);

    loop {
        match rx.recv() {
           Ok(event) => {
                match event {
                    DebouncedEvent::Write(_) => {
                        do_jit(&mut jit, &mut compile_duration, FILE_PATH);
                    },
                    _ => { }
                }
            },
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}

fn do_jit(jit: &mut Option<JitMemory>, compile_duration: &mut Option<Duration>, file_path: &str) {
    print!("\x1B[H\x1B[2J"); // clear console
    let mut file_str = String::new();
    let mut file = File::open(file_path).unwrap();
    file.read_to_string(&mut file_str).unwrap();

    if let Ok(ast) = syn::parse_file(&file_str) {
        let time_start = ::std::time::Instant::now();
        let compile_result = compiler::compile(ast);
        let time_end = ::std::time::Instant::now();
        if let Some(asm_buf) = compile_result {
            *compile_duration = Some(time_end - time_start);
            *jit = Some(JitMemory::from_assembly_buf(&asm_buf).unwrap());
        } else {
            println!("error: could not compile file");
        }
    } else {
        println!("error: could not parse file");
    }

    if let Some(ref mut jit_mem) = *jit {
        let result = (jit_mem.run())();
        println!("compiled in: {} ms", (compile_duration.unwrap().subsec_nanos()) as f32 / 1_000_000.0);
        println!("value is: {}", result);
        println!("value * 5 is: {}", 5 * result);
    } else {
        println!("compilation error");
    }
}