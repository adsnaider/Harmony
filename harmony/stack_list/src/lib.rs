#![cfg_attr(not(test), no_std)]
#![feature(naked_functions)]

use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, Ordering};

use private::Sealed;

#[repr(transparent)]
#[derive(Debug)]
pub struct StackNode<'a>(NonNull<StackNodeInner<'a>>);

#[repr(C)]
struct StackNodeInner<'a> {
    next: AtomicPtr<StackNodeInner<'a>>,
    size: usize,
    _buf: PhantomData<&'a ()>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StackNodeError {
    BufferNotAligned,
    BufferTooSmall,
}

impl<'a> StackNode<'a> {
    pub const REQUIRED_ALIGNEMENT: usize = 16;
    pub const NODE_SIZE: usize = core::mem::size_of::<StackNodeInner<'static>>();

    pub fn new(buffer: &'a mut [u8]) -> Result<Self, StackNodeError> {
        if buffer.as_ptr() as usize % Self::REQUIRED_ALIGNEMENT != 0 {
            return Err(StackNodeError::BufferNotAligned);
        }
        if buffer.len() < Self::NODE_SIZE {
            return Err(StackNodeError::BufferTooSmall);
        }
        let inner = StackNodeInner {
            next: AtomicPtr::new(core::ptr::null_mut()),
            size: buffer.len(),
            _buf: PhantomData,
        };
        let bytes: [u8; StackNode::<'static>::NODE_SIZE] = unsafe { core::mem::transmute(inner) };
        buffer[..bytes.len()].clone_from_slice(&bytes);
        Ok(Self(
            NonNull::new(buffer.as_mut_ptr() as *mut StackNodeInner<'a>).unwrap(),
        ))
    }

    pub fn into_buffer(self) -> &'a mut [u8] {
        unsafe {
            let size = self.0.as_ref().size;
            core::slice::from_raw_parts_mut(self.0.as_ptr() as *mut u8, size)
        }
    }

    unsafe fn from_inner(inner: NonNull<StackNodeInner<'a>>) -> Self {
        Self(inner)
    }

    fn inner(&self) -> NonNull<StackNodeInner<'a>> {
        self.0
    }
}

#[repr(transparent)]
pub struct StackList<'a> {
    head: AtomicPtr<StackNodeInner<'a>>,
}

impl<'a> StackList<'a> {
    pub const fn new() -> Self {
        Self {
            head: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    pub fn pop_front(&self) -> Option<StackNode<'a>> {
        match self
            .head
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |head| {
                if head.is_null() {
                    None
                } else {
                    unsafe { Some((*head).next.load(Ordering::SeqCst)) }
                }
            }) {
            Ok(stack) => Some(unsafe { StackNode::from_inner(NonNull::new_unchecked(stack)) }),
            Err(_) => None,
        }
    }

    pub fn push_front(&self, node: StackNode<'a>) {
        let node = node.inner();
        self.head
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |head| unsafe {
                node.as_ref().next.store(head, Ordering::SeqCst);
                Some(node.as_ptr())
            })
            .unwrap();
    }
}

#[macro_export]
macro_rules! stack_list_pop {
    () => {
        r#"
             movq    (%rdi), %rax
            33:
             testq   %rax, %rax
             je      34f
             movq    (%rax), %rcx
             lock    cmpxchgq %rcx, (%rdi)
             jne     33b
             jmp     35f
            34:
             xorl    %eax, %eax
            35:
        "#
    };
}

#[macro_export]
macro_rules! stack_list_push {
    () => {
        r#"
           movq    (%rdi), %rax
           36:
             movq    %rax, %rcx
             xchgq   %rcx, (%rsi)
             lock    cmpxchgq %rsi, (%rdi)
             jne     36b
        "#
    };
}

#[repr(align(16))]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct OveralignedU8(pub u8);

mod private {
    use crate::OveralignedU8;

    pub trait Sealed {}

    impl Sealed for [OveralignedU8] {}
}

pub trait AlignedU8Ext: Sealed {
    fn as_u8_slice(&self) -> &[u8];
    fn as_u8_slice_mut(&mut self) -> &mut [u8];
}

impl AlignedU8Ext for [OveralignedU8] {
    fn as_u8_slice(&self) -> &[u8] {
        unsafe { core::mem::transmute(self) }
    }

    fn as_u8_slice_mut(&mut self) -> &mut [u8] {
        unsafe { core::mem::transmute(self) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let stacks = StackList::new();

        let mut buf1 = [OveralignedU8(0); 256];
        let buf1_ptr = &buf1 as *const _ as *const u8;
        let mut buf2 = [OveralignedU8(0); 256];
        let buf2_ptr = &buf2 as *const _ as *const u8;
        let mut buf3 = [OveralignedU8(0); 256];
        let buf3_ptr = &buf3 as *const _ as *const u8;
        let n1 = StackNode::new(buf1.as_u8_slice_mut()).unwrap();
        let n2 = StackNode::new(buf2.as_u8_slice_mut()).unwrap();
        let n3 = StackNode::new(buf3.as_u8_slice_mut()).unwrap();
        stacks.push_front(n1);
        stacks.push_front(n2);
        stacks.push_front(n3);

        let stack = stacks.pop_front().unwrap().into_buffer();
        assert_eq!(stack.len(), 256);
        assert_eq!(stack.as_ptr() as *const _ as *const u8, buf3_ptr);
        let stack = stacks.pop_front().unwrap().into_buffer();
        assert_eq!(stack.len(), 256);
        assert_eq!(stack.as_ptr() as *const _ as *const u8, buf2_ptr);
        let stack = stacks.pop_front().unwrap().into_buffer();
        assert_eq!(stack.len(), 256);
        assert_eq!(stack.as_ptr() as *const _ as *const u8, buf1_ptr);
    }

    #[test]
    fn multi_threaded() {
        let stacks = StackList::new();
        let mut buffers = [[OveralignedU8(0); 256]; 16];
        std::thread::scope(|scope| {
            for buf in &mut buffers {
                let stack = StackNode::new(buf.as_u8_slice_mut()).unwrap();
                stacks.push_front(stack);
            }
            for i in 0..16 {
                let stacks = &stacks;
                scope.spawn(move || {
                    let stack = stacks.pop_front().unwrap().into_buffer();
                    for _ in 0..8 {
                        for e in stack.iter_mut() {
                            *e = i;
                        }
                    }
                });
            }
        })
    }
}
