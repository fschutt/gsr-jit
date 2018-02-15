// x86_64 gives some guarantees that other architectures give not
pub enum Register {
    // Accumulator, { Eax, Ax, Ah, Al }
    Rax,
    // Base, { Ebx, Bx, Bh, Bl }
    Rbx,
    // Counter, { Ecx, Cx, Ch, Cl }
    Rcx,
    // Data, { Ecx, Cx, Ch, Cl }
    Rdx,
    // Stack pointer
    Rsp, 
    // Base pointer
    Rbp,
    // Source index
    Rsi,
    // Destination index
    Rdi,
    R7,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    Xmm0,
    Xmm1,
    Xmm2,
    Xmm3,
    Xmm4,
    Xmm5,
    Xmm6,
    Xmm7,
    Xmm8,
    Xmm9,
    Xmm10,
    Xmm11,
    Xmm12,
    Xmm13,
    Xmm14,
    Xmm15,
}

impl Register {
    pub fn get_width(&self) -> u8 {
        // TODO
        0
    }
}

pub enum Syscall {
    Linux(LinuxSyscall),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LinuxSyscall {
    Write(LinuxWriteSyscall),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LinuxWriteSyscall {
    pub target_file_descriptor: u32,
    pub source_bytes: *const u8,
    pub byte_count: u32,
}

pub trait GetSyscallOpCode64 {
    fn get_rax(&self) -> Option<u64>;
    fn get_rbx(&self) -> Option<u64>;
    fn get_rcx(&self) -> Option<u64>;
    fn get_rdx(&self) -> Option<u64>;
}

pub trait GetSyscallOpCode32 {
    fn get_eax(&self) -> Option<u32>;
    fn get_ebx(&self) -> Option<u32>;
    fn get_ecx(&self) -> Option<u32>;
    fn get_edx(&self) -> Option<u32>;
}

impl GetSyscallOpCode64 for LinuxWriteSyscall {
    fn get_rax(&self) -> Option<u64> {
        Some(0x04)
    }
    fn get_rbx(&self) -> Option<u64> {
        Some(self.target_file_descriptor as u64)
    }
    fn get_rcx(&self) -> Option<u64> {
        Some(self.source_bytes as u64)
    }
    fn get_rdx(&self) -> Option<u64> {
        Some(self.byte_count as u64)
    }
}

impl GetSyscallOpCode32 for LinuxWriteSyscall {
    fn get_eax(&self) -> Option<u32> {
        Some(0x04)
    }
    fn get_ebx(&self) -> Option<u32> {
        Some(self.target_file_descriptor)
    }
    fn get_ecx(&self) -> Option<u32> {
        Some(self.source_bytes as u32)
    }
    fn get_edx(&self) -> Option<u32> {
        Some(self.byte_count)
    }
}