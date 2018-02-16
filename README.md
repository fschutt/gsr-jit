# gsr-jit

## Notice

For now, this was only a test, this repository isn't further maintained.

## What is GSR?

This repository was supposed to be a "game-specific Rust compiler", i.e. a 
JIT compiler that would simply parse a Rust module and translate it to assembly,
then execute it (when a game level is loaded). This is important because:

- In larger game engines you don't want to re-compile the whole engine just for every minimal change (faster iteration)
- You want certain mathematical operation / vector operations to use specific assembly instructions, but you only know
  what CPU features you can use at runtime, so either you write duplicated code or you use JIT on the target system.
- Interpreters such as Lua can incur significant overhead when calling across FFI boundaries (function lookup)

GSR can compile a file in less then a millisecond, which is important if you want fast iteration.
I tried using the official Rust compiler for this and using LLVM, but it doesn't work. LLVM expects you to 

## Syntax

GSR uses the `syn` parser, adhering to the regular Rust syntax. Currently it can only compile 
functions that return integers, just as a test. GSR loads the file, then looks for the `#[start]` attribute, 
which is the program entry point. It assembles the dependent files into assembly **without any optimization**.
Then it allocates memory pages for executable memory and jumps to the begin of the page and executes.

Example:

```rust
// in to_be_ji_compiled.rs
#[start]
fn my_main_function() -> u32 {
    500
}
```
```rust 
// in main.rs
// this can be done in a loop, at runtime, for hot-reloading code
let ast = syn::parse_file(include_str!("to_be_ji_compiled.rs")).unwrap();
// assemble the AST into asm opcodes
let assembly_instructions = compiler::compile(ast); 
// load the executable memory and load the assembly into it
let jit = JitMemory::from_assembly_buf(&assembly_instructions).unwrap();
// tell the CPU to jump to the entry function and start executing
let result = (jit.run())();
println!("the returned number is: {}", result); // prints "500"
```

What GSR currently does:

- It checks that the return type of the function is the same return type of the last expression
- It uses the `movabs` instructions only if a 64-bit integer is necessary.

## Goals and non-goals

GSR does not aim to be a general-purpose JIT compiler, rather it aims to use the regular Rust syntax 
for "gameplay scripting". There should be no generics support or large optimizatiosn, for example:
it's purely for simple gameplay scripting, not large libraries. There is also no dependency management and
`extern crate` is forbidden: The goal is to make levels playable, where the AOT-compiled game engine
provides an API which the JIT-compiled code can then call into. modules are allowed, in order to split 
functionality across files, but extern libraries are forbidden, because each "level" is just one start module 
with an entry function and from there on the functions are executed accordingly. 

GSR should know about special mathematical optimizations, specifically SIMD and vector instructions.
These should be JIT-compiled on the target users CPU, according to the features that the CPU supports.
A secondary goal is to integrate the JIT with an Entity-Component-System such as SPECS. This would make it ideal for
defining the data models ahead of time, but tweaking the behaviour at runtime. A third goal is to make
GSR available for modding, but check that the code is not doing anything malicious (no reading or writing files,
those have to be called from the game engine).
