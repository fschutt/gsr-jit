extern crate notify;
extern crate gsr_jit;

use notify::{Watcher, RecursiveMode, DebouncedEvent, watcher};
use std::{sync::mpsc::channel, time::Duration, fs::read_to_string};
use gsr_jit::{JitMemory, parse_file, compile};

fn main() {

    // relative to the root directory
    let file_path = "./tests/simple.rs";

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_millis(200)).unwrap();

    watcher.watch(file_path, RecursiveMode::Recursive).unwrap();

    let mut file = read_to_string(file_path).unwrap();
    let mut jit_mem = None;

    clear_console();
    assemble(&mut jit_mem, &file);
    println!("{}", exec::<u64>(&jit_mem.as_ref().unwrap()));

    loop {
        match rx.recv() {
            Ok(DebouncedEvent::Write(_)) => {
                clear_console();
                file = read_to_string(file_path).unwrap();
                assemble(&mut jit_mem, &file);
                println!("{}", exec::<u64>(&jit_mem.as_ref().unwrap()));
            },
            Ok(_) => { },
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}

fn clear_console() {
    print!("\x1B[H\x1B[2J");
}

fn assemble(jit: &mut Option<JitMemory>, file_str: &str) {
    let ast = parse_file(file_str).unwrap();
    let assembly_buf = compile(ast).unwrap();
    *jit = Some(JitMemory::from_assembly_buf(&assembly_buf).unwrap());
}

fn exec<T>(mem: &JitMemory) -> T {
    mem.run::<T>()()
}