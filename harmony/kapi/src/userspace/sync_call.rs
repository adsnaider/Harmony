pub use stack_list;

#[macro_export]
macro_rules! sync_call {
    ($name:ident, $stacks:path, $fun:expr) => {
        #[naked]
        extern "C" fn $name(_a: usize, _b: usize, _c: usize, _d: usize) -> isize {
            extern "C" fn inner(a: usize, b: usize, c: usize, d: usize) -> isize {
                $fun(a, b, c, d)
            }

            use stack_list::{stack_list_pop, stack_list_push};
            unsafe {
                core::arch::asm!(
                    "movq %rdi, %r12",
                    "movq %rsi, %r13",
                    "movq %rdx, %r14",
                    "movq %rcx, %r15",
                    "movq ${stacks}, %rdi",
                    stack_list_pop!(),
                    "testq %rax, %rax",
                    "je   1f",
                    "movq %r12, %rdi",
                    "movq %r13, %rsi",
                    "movq %r14, %rdx",
                    "movq %r15, %rcx",
                    "movq 8(%rax), %r12",
                    "leaq (%rax, %r12, 1), %rsp",
                    "call {inner}",
                    "movq %rax, %r13", // save result
                    "movq ${stacks}, %rdi", // arg0
                    "movq %rsp, %rsi", // arg1 is our bottom of stack
                    "subq %r12, %rsi",
                    "movq %r12, 8(%rsi)", // Reset the stack node to include the size.
                    stack_list_push!(),
                    "movq %r13, %rax",
                    "jmp 2f",
                    "1:",
                     "movq $-1, %rax",
                    "2:",
                     "movq $0, %rsp",
                     "movq $0, %rdi",
                     "movq $15, %rsi",
                     "movq %rax, %rdx",
                    "int $0x80",
                    "ud2",
                    stacks = sym $stacks,
                    inner = sym inner,
                    options(noreturn, att_syntax),
                )
            }
        }
    };
}
