use syn::File;
use syn::Stmt;
use syn::Type;
use syn::FnArg;
use syn::Item::Fn;
use syn::ReturnType;
use syn::ItemFn;
use syn::Ident;
use syn::Path;
use syn::Lit;
use syn::Expr;
use syn::IntSuffix;

use std::collections::{HashSet, BTreeMap};
use std::fmt;

pub type FnMap = BTreeMap<GlobalLabel, Function>;
pub type FnOffsetMap = BTreeMap<GlobalLabel, FnLocation>;

const FN_PROLOGUE: [u8;4] = [
0x55,                     // push   rbp
0x48, 0x89, 0xE5          // mov    rbp,rsp
];

const FN_EPILOGUE: [u8;2] = [
0x5D,                     // pop    rbp
0xC3                      // ret
];

static mut GLOBAL_LABEL_ID: usize = 0;

pub struct AssemblyBuf {
    pub instructions: Vec<u8>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AllocationError {
    /// Instructions are too big to fit in the allocated JIT memory
    InstructionBufTooLarge,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Ret {
    Str,
    ByteStr,
    Byte,
    Char,
    Int(StaticIntLiteral),
    Float(StaticFloatLiteral),
    Bool,
    Vec(StaticVecLiteral),
}

pub enum Instruction {
    OneComponent(u8),
    TwoComponent((u8, u8))
}
impl Ret {
    pub fn get_optimal_register_return(&self) -> Option<Instruction> {
        use self::Ret::*;
        use self::StaticIntLiteral::*;
        match *self {
            Int(i) => {
                match i {
                    I8 | U8 => Some(Instruction::OneComponent(0xB0)), // mov al [0x04]
                    I16 | U16 => Some(Instruction::TwoComponent((0x66, 0xB8))), // mov ax [0x04, 0x00]
                    I32 | U32 => Some(Instruction::OneComponent(0xB8)), // mov eax [0x04, 0x00, 0x00, 0x00]
                    I64 | U64 => Some(Instruction::TwoComponent((0x48, 0xB8))), // movabs rax [0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
                    _ => None,
                }
            },
            _ => None
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum StaticFloatLiteral {
    F64,
    F32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum StaticIntLiteral {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    UnknownSize(u64)
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum StaticVecLiteral {
    Vec2,
    Vec3,
    Vec4,
}

#[derive(Debug, Hash, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct GlobalLabel(pub usize);

impl fmt::Display for GlobalLabel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, ".LBB{}", self.0)
    }
}

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct Function {
    pub name: FnName,
    pub arguments: Vec<FnArg>,
    pub statements: Vec<Stmt>,
    pub return_type: Option<Type>,
    pub memory_location: Option<AssemblyOffset>,
}

impl Function {
    fn display(&self) -> String {
        format!("{} {{ arguments: {}, statements: {}, return_type: {} }}",
            self.name, self.arguments.len(), self.statements.len(), self.return_type.is_some())
    }
}

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct FnName(pub Ident);

impl fmt::Display for FnName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "fn {}", self.0)
    }
}

pub fn compile(ast: File) -> Option<AssemblyBuf> {    
    let mut entry_fn: Option<GlobalLabel> = None;
    let mut module_functions = BTreeMap::<GlobalLabel, Function>::new();
    
    {     
        let mut module_functions_set = HashSet::<FnName>::new();
        for item in ast.items {
            match item {
                Fn(f) => {
                    let fn_name = FnName(f.ident.clone());
                    let fn_label = GlobalLabel(unsafe { GLOBAL_LABEL_ID });
                    if module_functions_set.contains(&fn_name) {
                        println!("error: function {:?} declared multiple times", fn_name);
                        return None;
                    } else {
                        module_functions_set.insert(fn_name.clone());
                    }

                    let return_type: Option<Type> = match f.decl.output {
                        ReturnType::Default => None,
                        ReturnType::Type(_, ref t) => Some((*(*t)).clone())
                    };

                    let statements = f.block.stmts.clone();
                    let arguments = f.decl.inputs.iter().cloned().collect();

                    let result_fn = Function {
                        name: fn_name,
                        arguments: arguments,
                        statements: statements,
                        return_type: return_type,
                        memory_location: None,
                    };
                    module_functions.insert(fn_label, result_fn);
                    if is_start_label(&f) {
                        if entry_fn.is_some() {
                            println!("error: #[start] declared multiple times");
                            return None;
                        } else {
                            entry_fn = Some(fn_label);
                        }
                    }
                    unsafe { GLOBAL_LABEL_ID += 1 };
                },
                _ => { }
            }
        }
    }

    println!("\nfunctions in this module:\n");
    for mod_fn in &module_functions {
        println!("\t{}: {}", mod_fn.0, mod_fn.1.display());
    }

    if let Some(entry_function) = entry_fn {
        println!("\nentry point: {}\n", entry_function);
        let mut fn_offset_map = FnOffsetMap::new();
        // at the start, we don't know the locations of the functions in memory
        for (label, mod_fn) in module_functions.iter() {
            fn_offset_map.insert(*label, FnLocation::UnresolvedFnName(mod_fn.name.clone()));
        }
        Some(AssemblyBuf {
            instructions: assemble_function(entry_function, &mut module_functions, &mut fn_offset_map),
        })
    } else {
        None
    }
}

fn has_first_segment(path: &Path, expected: &'static str) -> bool {
    path.segments.first().and_then(|segment|
        if segment.value().ident == Ident::from(expected) { 
            Some(()) 
        } else { 
            None
        }).is_some()
}

fn get_first_segment(path: &Path) -> Option<Ident> {
    path.segments.first().and_then(|segment| Some(segment.value().ident))
}

fn is_start_label(f: &ItemFn) -> bool {
    f.attrs.iter().any(|attr| 
        attr.path.leading_colon.is_none() &&
        has_first_segment(&attr.path, "start"))
}

#[derive(Debug, Hash, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct AssemblyOffset(pub usize);

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum FnLocation {
    MemoryOffset(AssemblyOffset),
    UnresolvedFnName(FnName),
}

pub enum AssembleError {
    /// Mismatch between the body of the function
    ReturnTypeMismatch
}

fn assemble_function(fn_location: GlobalLabel, fn_map: &mut FnMap, fn_offset_map: &mut FnOffsetMap) -> Vec<u8> {

    // what are the offsets of the label into the assembly 
    // (offsetfrom the start of the memory)
    let entry = fn_map.get(&fn_location).unwrap();
    let mut instructions = Vec::with_capacity(6);
    instructions.extend_from_slice(&FN_PROLOGUE);

    let return_type_outer = get_return_type_outer(entry.return_type.as_ref());
    let return_type_inner = get_return_type_inner(&entry.statements, &return_type_outer);

    if return_type_outer != return_type_inner {
        println!("error: return types don't match in {}", entry.name);
        return Vec::new();
    }

    instructions.append(&mut assemble_statements(&entry.statements, return_type_outer, fn_map));
    instructions.extend_from_slice(&FN_EPILOGUE);
    instructions
}

fn get_return_type_inner(statements: &Vec<Stmt>, expected_type: &Option<Ret>) -> Option<Ret> {
    // this will need a lot of work to work correctly

    // TODO: check for early-return statements

    // check last expression
    let last_statement = statements.last()?;
    match *last_statement {
        Stmt::Expr(ref e) => {
            if let Some(ref expected) = *expected_type {
                match *e {
                    Expr::Lit(ref l) => match l.lit {
                        Lit::Int(ref i) => {
                            if let Ret::Int(ref expected_int) = *expected {
                                match i.suffix() {
                                    IntSuffix::None => return try_match_u64_value(i.value(), expected_int),
                                    IntSuffix::I8 => return Some(Ret::Int(StaticIntLiteral::I8)),
                                    IntSuffix::I16 => return Some(Ret::Int(StaticIntLiteral::I16)),
                                    IntSuffix::I32 => return Some(Ret::Int(StaticIntLiteral::I32)),
                                    IntSuffix::I64 => return Some(Ret::Int(StaticIntLiteral::I64)),
                                    IntSuffix::U8 => return Some(Ret::Int(StaticIntLiteral::U8)),
                                    IntSuffix::U16 => return Some(Ret::Int(StaticIntLiteral::U16)),
                                    IntSuffix::U32 => return Some(Ret::Int(StaticIntLiteral::U32)),
                                    IntSuffix::U64 => return Some(Ret::Int(StaticIntLiteral::U64)),
                                    _ => { },
                                }
                            }
                        },
                        _ => { }
                    },
                    _ => { },
                }
            }
        },
        _ => { },
    }

    None
}

fn try_match_u64_value(actual: u64, expected: &StaticIntLiteral) -> Option<Ret> {
    let minimal_size = determine_minimal_size(actual);

    match *expected {
        StaticIntLiteral::U64 => { return Some(Ret::Int(*expected)); }
        StaticIntLiteral::U32 => {
            if minimal_size == StaticIntLiteral::U32 ||
               minimal_size == StaticIntLiteral::U16 ||
               minimal_size == StaticIntLiteral::U8 {
                return Some(Ret::Int(*expected))
            }
        },
        StaticIntLiteral::U16 => {
            if minimal_size == StaticIntLiteral::U16 ||
               minimal_size == StaticIntLiteral::U8 {
                return Some(Ret::Int(*expected))
            }
        },
        StaticIntLiteral::U8 => {
            if minimal_size == StaticIntLiteral::U8 {
                return Some(Ret::Int(*expected))
            }
        },
        _ => { }
    }
    
    println!("warn: value: {:?} doesn't fit in return value!", actual);
    None
}

fn determine_minimal_size(actual: u64) -> StaticIntLiteral {
    if actual < 0xff {
        StaticIntLiteral::U8
    } else if actual < 0xffff {
        StaticIntLiteral::U16
    } else if actual < 0xffffffff {
        StaticIntLiteral::U32
    } else {
        StaticIntLiteral::U64
    }
} 

fn get_return_type_outer(return_type: Option<&Type>) -> Option<Ret> {
    use syn::Type;
    use syn::Ident;

    let return_type = return_type?;
    match *return_type {
        Type::Path(ref p) => {
            if p.path.leading_colon.is_some() {
                return None;
            }
            let first_segment = get_first_segment(&p.path)?;
            if first_segment == Ident::from("u8") {
                Some(Ret::Int(StaticIntLiteral::U8))
            } else if first_segment == Ident::from("u16") {
                Some(Ret::Int(StaticIntLiteral::U16))
            } else if first_segment == Ident::from("u32") {
                Some(Ret::Int(StaticIntLiteral::U32))
            } else if first_segment == Ident::from("u64") {
                Some(Ret::Int(StaticIntLiteral::U64))
            } else {
                None
            }
        },
        _ => None
    }
}

fn assemble_statements(stmts: &Vec<Stmt>, return_type: Option<Ret>, fn_map: &FnMap) -> Vec<u8> {
    let mut assembly_vec = Vec::<u8>::new();
    let return_type = return_type.unwrap();
    for stmt in stmts {
        match *stmt {
            Stmt::Expr(ref e) => match *e {
                Expr::Lit(ref l) => match l.lit {
                    Lit::Int(ref i) => {
                        let val = i.value();
                        let min_size = determine_minimal_size(val);
                        let mut optimal_return_size = return_type;
                        if return_type == Ret::Int(StaticIntLiteral::U64) {
                            if min_size == StaticIntLiteral::U32 || 
                               min_size == StaticIntLiteral::U16 || 
                               min_size == StaticIntLiteral::U8 {
                                optimal_return_size = Ret::Int(StaticIntLiteral::U32);
                            }
                        }
                        let asm_instr = optimal_return_size.get_optimal_register_return();
                        if let Some(asm_instr) = asm_instr {                 
                            match asm_instr {
                                Instruction::OneComponent(a) => {
                                    assembly_vec.push(a);
                                },
                                Instruction::TwoComponent((a, b)) => { 
                                    assembly_vec.push(a);
                                    assembly_vec.push(b);
                                },
                            }

                            match optimal_return_size {
                                Ret::Int(i) => {
                                    match i {
                                        StaticIntLiteral::U64 => assembly_vec.extend_from_slice(&transform_u64_to_array_of_u8_le(val)),
                                        StaticIntLiteral::U32 => assembly_vec.extend_from_slice(&transform_u32_to_array_of_u8_le(val as u32)),
                                        StaticIntLiteral::U16 => assembly_vec.extend_from_slice(&transform_u16_to_array_of_u8_le(val as u16)),
                                        StaticIntLiteral::U8 => assembly_vec.push(val as u8),
                                        _ => { },
                                    }
                                },
                                _ => { /* do nothing for now*/}
                            }
                        }
                    },
                    _ => { },
                },
                _ => { },
            },
            _ => { }
        }
    }

    assembly_vec
}

fn transform_u32_to_array_of_u8_le(x:u32) -> [u8;4] {
    let b1 : u8 = ((x >> 24) & 0xff) as u8;
    let b2 : u8 = ((x >> 16) & 0xff) as u8;
    let b3 : u8 = ((x >> 8) & 0xff) as u8;
    let b4 : u8 = (x & 0xff) as u8;
    [b4, b3, b2, b1]
}

fn transform_u16_to_array_of_u8_le(x:u16) -> [u8;2] {
    let b1 : u8 = ((x >> 8) & 0xff) as u8;
    let b2 : u8 = (x & 0xff) as u8;
    [b2, b1]
}

// -5394849584509 => 0x83, 0x2A, 0xE8, 0xE9, 0x17, 0xFB, 0xFF, 0xFF

// 0x7D, 0xD5, 0x17, 0x16, 0xE8, 0x04, 0x00, 0x00
fn transform_u64_to_array_of_u8_le(x:u64) -> [u8;8] {
    let b1 : u8 = ((x >> 56) & 0xff) as u8;
    let b2 : u8 = ((x >> 48) & 0xff) as u8;
    let b3 : u8 = ((x >> 40) & 0xff) as u8;
    let b4 : u8 = ((x >> 32) & 0xff) as u8;
    let b5 : u8 = ((x >> 24) & 0xff) as u8;
    let b6 : u8 = ((x >> 16) & 0xff) as u8;
    let b7 : u8 = ((x >> 8) & 0xff) as u8;
    let b8 : u8 = (x & 0xff) as u8;
    [b8, b7, b6, b5, b4, b3, b2, b1]
}