# Memory Management

Harmony takes a somewhat drastic approach to memory management: "Let's not".
That's right, Harmony provides basically no memory management to userspace,
instead, userspace is required to manage its own memory and the kernel's. This
idea can feel strange since the Kernel **cannot** trust userspace. But such
an approach can work and has been implemented in kernels such as SeL4 and 
Composite.

## How does it work?

The idea is simple: Type physical frames as either user, kernel, or untyped.
The kernel maintains a reference count for each frame and transitions from one
type to another can only happen when the reference count is 0. Specifically,
the kernel provides the syscall to retype frames from User &harr; Untyped &harr;
Kernel.

In this way, we can guarantee that no user-level component has access to
physical frames that are used for kernel datastrctures.

## Kernel Memory

### What about malloc?

A concequence of this, is that our kernel has no heap. We don't need it nor want
it. Having a heap (a.k.a. malloc/free) makes it almost impossible to provide
a wait-free kernel. It also means that the kernel would have to track kernel
memory usage per component. That'd be unnecessary policy in the kernel.

However, the kernel will often need to allocate kernel memory such as for new
threads, page tables, capability tables, etc.

### Allocating Kernel Datastructures

As mentioned, the Harmony kernel has no heap. Instead, every kernel
datastructure that we need to allocate has one requirement: Sized and aligned to
a physical frame.

Page tables are like so by hardware design. The thread control blocks are
smaller than a page but we simply extend that. Capability tables are built up
to span exactly one page each. Whenever a user-level component wants to create
one of these data structures, they need to pass in the system call the physical
frame they want to use. And if the frame is properly typed and the component
has access to this frame, then the kernel wil use it to allocate the new data
structure.

## User memory

User memory is pretty simple: If a frame is typed as user, the kernel will
enable page table operations to map the frame.
