use syn::File;
use syn::Stmt;
use syn::Type;
use syn::FnArg;
use syn::Item::Fn;
use syn::ReturnType;
use syn::ItemFn;
use syn::Ident;
use std::collections::{HashSet, BTreeMap};

static mut GLOBAL_LABEL_ID: usize = 0;

pub struct AssemblyBuf {
    pub instructions: Vec<u8>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AllocationError {
    /// Instructions are too big to fit in the allocated JIT memory
    InstructionBufTooLarge,
}

pub enum Ret {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Vec2,
    Vec3,
    Vec4,
}

#[derive(Debug, Hash, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct GlobalLabel(pub usize);
use std::fmt;
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
                        break;
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
                    };
                    module_functions.insert(fn_label, result_fn);
                    if is_start_label(&f) {
                        if entry_fn.is_some() {
                            println!("error: #[start] declared multiple times");
                            break;
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

    println!("\nfunctions in this module:\n", );
    for mod_fn in &module_functions {
        println!("\t{}: {}", mod_fn.0, mod_fn.1.display());
    }

    if let Some(entry) = entry_fn {
        println!("\nentry point: {}\n", entry);

        Some(create_assembly(entry, module_functions))
    } else {
        None
    }
}

fn is_start_label(f: &ItemFn) -> bool {
    f.attrs.iter().any(|attr| 
        attr.path.leading_colon.is_none() &&
        attr.path.segments.first().and_then(|segment|
            if segment.value().ident == Ident::from("start") { 
                Some(()) 
            } else { 
                None
            }).is_some())
}

type FnMap = BTreeMap::<GlobalLabel, Function>;
pub struct AssemblyOffset(pub usize);

const ASM_RET: u8 = 0xC3;
const FN_PROLOGUE: [u8;4] = [
0x55,                     // push   rbp
0x48, 0x89, 0xE5          // mov    rbp,rsp
];

const FN_EPILOGUE: [u8;2] = [
0x5D,                     // pop    rbp
0xC3                      // ret
];

fn create_assembly(entry_fn: GlobalLabel, fn_map: FnMap) -> AssemblyBuf {
    // what are the offsets of the label into the assembly (from the start)
    let mut fn_offset_map = BTreeMap::<GlobalLabel, AssemblyOffset>::new();
    let entry = fn_map.get(&entry_fn).unwrap();
    println!("assembling function: {}", entry.name);
    for statement in &entry.statements {
        println!("{:?}", statement);
    }

    let instructions = vec![
        0x55,                     // push   rbp
        0x48, 0x89, 0xE5,         // mov    rbp,rsp
        0xB0, 0x04,               // mov    al,0x4
        0x5D,                     // pop    rbp
        0xC3,                     // ret
    ];

    AssemblyBuf {
        instructions: instructions,
    }
}