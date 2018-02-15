pub struct AssemblyBuf {
    pub instructions: Vec<u8>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AllocationError {
    /// Instructions are too big to fit in the allocated JIT memory
    InstructionBufTooLarge,
}