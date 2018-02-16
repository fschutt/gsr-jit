use compiler::{AssemblyBuf, AllocationError};
use std::ptr;
use std::ops::{Index, IndexMut};
use libc;
use page_size;

#[derive(Debug)]
pub struct JitMemory {
    /// The page size at time of allocation
    page_size: usize,
    /// How many memory pages were allocated
    number_of_pages: usize,
    /// Total allocated size (page_size * number_of_pages)
    allocated_size: usize,
    /// Pointer to the memory
    memory_ptr: *mut u8,
}

struct JitSetup {
    page_size: usize,
    allocation_size_in_bytes: usize,
    memory_ptr: *mut libc::c_void,
}

impl JitMemory {

    fn pre_setup(num_pages: usize) -> JitSetup {
        let page_size = page_size::get();
        let allocation_size_in_bytes = num_pages * page_size;
        let ptr = ptr::null_mut();
        JitSetup {
            page_size: page_size,
            allocation_size_in_bytes: allocation_size_in_bytes,
            memory_ptr: ptr,
        }
    }

    pub fn from_assembly_buf(assembly: &AssemblyBuf) -> Option<Self> {
        let page_size = page_size::get();
        let buf_len = assembly.instructions.len();
        let necessary_pages = (buf_len as f32 / page_size as f32).ceil() as usize;
        let mut memory = Self::new(necessary_pages)?;
        memory.load_assembly(assembly).ok()?;
        Some(memory)
    }

    #[cfg(target_os = "linux")]
    fn new(num_pages: usize) -> Option<Self> {
        let JitSetup { page_size, allocation_size_in_bytes, mut memory_ptr } = 
            Self::pre_setup(num_pages);
        
        let alloc_error = unsafe {
          libc::posix_memalign(&mut memory_ptr, page_size::get(), allocation_size_in_bytes)
        };

        match alloc_error {
            libc::ENOMEM => { 
                println!("recieved ENOMEM: no memory avaliable anymore");
                return None;
            },
            libc::EINVAL => { 
                println!("recieved EINVAL: memory allocation not power of two"); 
                return None; 
            },
            _ => { },
        }

        if memory_ptr.is_null() {
            println!("posix_memalign failed for some unknown reason");
            return None;
        }

        let mprotect_err = unsafe {
            libc::mprotect(memory_ptr, allocation_size_in_bytes, 
                           libc::PROT_EXEC | libc::PROT_READ | libc::PROT_WRITE)
        };

        if mprotect_err == -1 {
            println!("mprotect failed!");
            unsafe { libc::free(memory_ptr) };
            return None;
        }

        // memset(3) should return the original pointer again
        // It is not important if this function actually succeeds,
        // if it doesn't, the pages are uninitialized
        let ptr_memory_area = unsafe { libc::memset(memory_ptr, 0xCC, allocation_size_in_bytes) };
        if ptr_memory_area as usize != memory_ptr as usize {
            println!("warning: memset error!");
        }

        Some(JitMemory {
            number_of_pages: num_pages,
            page_size: page_size,
            allocated_size: allocation_size_in_bytes,
            memory_ptr: memory_ptr as *mut u8,
        })
    }
    
    #[cfg(target_os = "windows")]
    fn new(num_pages: usize) -> Option<Self> {
        use winapi::um::memoryapi::{VirtualProtect, VirtualAlloc};
        use winapi::um::winnt::{MEM_RESERVE, MEM_COMMIT, PAGE_EXECUTE_READWRITE};
        
        let JitSetup { page_size, allocation_size_in_bytes, mut memory_ptr } = 
            Self::pre_setup(num_pages);

        let memory_ptr = VirtualAlloc(0, allocation_size_in_bytes, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
        if memory_ptr.is_null() {
            println!("VirtualAlloc failed!");
            return None;
        }

        let virtualprotect_err = unsafe {
            VirtualProtect(memory_ptr, allocation_size_in_bytes, PAGE_EXECUTE_READWRITE, &mut 0 as *mut i32)
        };

        if virtualprotect_err == 0 {
            println!("VirtualProtect failed!");
            unsafe { libc::free(memory_ptr) };
            return None;
        }

        Some(JitMemory {
            number_of_pages: num_pages,
            page_size: page_size,
            allocated_size: allocation_size_in_bytes,
            memory_ptr: memory_ptr as *mut u8,
        })
    }

    pub fn get(&self, index: usize) -> Option<&u8> {
        if index > self.allocated_size { 
            None
        } else {
            Some(unsafe { self.get_unchecked(index) })
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut u8> {
        if index > self.allocated_size { 
            None
        } else {
            Some(unsafe { self.get_unchecked_mut(index) })
        }
    }

    /// Returns a pointer to the element at the given index, without doing bounds checking.
    pub unsafe fn get_unchecked(&self, index: usize) -> &u8 {
        &*self.memory_ptr.offset(index as isize)
    }

    /// Returns an unsafe mutable pointer to the element in index
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut u8 {
        &mut *self.memory_ptr.offset(index as isize)
    }

    // Dump the JIT memory in hex
    pub fn dump_mem(&self) {
        use std::fmt::Write;
        let mut s = String::with_capacity(self.allocated_size);
        let mut page_counter = 0;
        for i in 0..self.allocated_size {
            if i > 160 { break; }
            if i % self.page_size == 0 {
                let page_start = unsafe { self.memory_ptr.offset((page_counter * self.page_size) as isize) };
                write!(&mut s, "\n>>>>> JIT memory - page {} @ 0x{:x}\n", page_counter, page_start as usize).unwrap();
                page_counter += 1;
            }
            if i != 0 && i % 16 == 0 {
                write!(&mut s, "\n").unwrap();
            }
            write!(&mut s, "{:02x} ", self[i]).unwrap();
        }
        println!("{}", s);
    }

    pub fn load_assembly(&mut self, data: &AssemblyBuf) -> Result<(), AllocationError> {
        let instructions_len = data.instructions.len();
        if instructions_len > self.allocated_size {
            Err(AllocationError::InstructionBufTooLarge)
        } else {
            unsafe { ptr::copy(data.instructions.as_ptr(), self.memory_ptr, instructions_len) };
            Ok(())   
        }
    }

    pub fn run(&mut self) -> (fn() -> u64) {
        unsafe { ::std::mem::transmute(self.memory_ptr) }
    }
}

impl Index<usize> for JitMemory {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        #[cfg(debug_assertions)]
        return self.get(index).unwrap();

        #[cfg(not(debug_assertions))]
        return unsafe { self.get_unchecked(index) };
    }
}

impl IndexMut<usize> for JitMemory {
    fn index_mut(&mut self, index: usize) -> &mut u8 {
        #[cfg(debug_assertions)]
        return self.get_mut(index).unwrap();

        #[cfg(not(debug_assertions))]
        return unsafe { self.get_unchecked_mut(index) };
    }
}

impl Drop for JitMemory {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.memory_ptr as *mut libc::c_void);
        }
    }
}
