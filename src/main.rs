extern crate libc;
extern crate page_size;

use std::ptr;
use std::ops::{Index, IndexMut};

extern {
    fn memset(s: *mut libc::c_void, c: libc::uint32_t, n: libc::size_t) -> *mut libc::c_void;
}

/// breakpoint / halt asm instruction
const ASM_BREAK: libc::uint32_t = 0xCC;

#[derive(Debug)]
struct JitMemory {
    /// The page size at time of allocation
    page_size: usize,
    /// How many memory pages were allocated
    number_of_pages: usize,
    /// Total allocated size (page_size * number_of_pages)
    allocated_size: usize,
    /// Pointer to the memory
    memory_ptr: *mut u8,
}

impl JitMemory {
    pub fn new(num_pages: usize) -> Option<JitMemory> {
        
        let page_size = page_size::get();

        let allocation_size_in_bytes = num_pages * page_size;
        let mut memory_ptr: *mut libc::c_void = ptr::null_mut();
        
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

        let mprotect_err;
        unsafe {
/*
            mprotect_err = libc::mprotect(memory_ptr, allocation_size_in_bytes, 
                               libc::PROT_EXEC | libc::PROT_READ | libc::PROT_WRITE);
*/
            mprotect_err = libc::mprotect(memory_ptr, allocation_size_in_bytes, 
                               libc::PROT_EXEC | libc::PROT_WRITE);
        }

        if mprotect_err == -1 {
            println!("mprotect failed!");
            return None;
        }

        // memset(3) should return the original pointer again
        // It is not important if this function actually succeeds,
        // if it doesn't, the pages are uninitialized
        let ptr_memory_area = unsafe { memset(memory_ptr, ASM_BREAK, allocation_size_in_bytes) };
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
        for i in 0..self.allocated_size {
            if i != 0 && i % 32 == 0 {
                write!(&mut s, "\n").unwrap();
            }
            write!(&mut s, "{:02x} ", self[i]).unwrap();
        }
        println!("{}", s);
    }

    pub fn get_entry_point_fn(&mut self) -> (fn() -> u8) {
        assert!(self.allocated_size > 6);

        self[0] = 0x55;                                         // push   rbp
        self[1] = 0x48; self[2] = 0x89; self[3] = 0xE5;         // mov    rbp,rsp
        self[4] = 0xB0; self[5] = 0x04;                         // mov    al,0x4
        self[6] = 0x5D;                                         // pop    rbp
        self[7] = 0xC3;                                         // ret

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

fn main() {
    let mut jit = JitMemory::new(1).unwrap();
    
    let entry_point = jit.get_entry_point_fn();

    // should print "4"
    println!("the returned value is: {:x}", entry_point());

    println!("memory dump:");
    // THIS SHOULD NOT WORK IF mprotect(PROT_READ) isn't set!
    jit.dump_mem(); 
}
