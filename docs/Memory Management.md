# Memory Retyping

The idea is simple, the implementation not so much: Let userspace manage all physical memory.

## Purpose

We want to remove complexity from the kernel. The moment you have a memory allocator, is the moment you require locking primitives in the kernel. Allocating in the kernel also means that the kernel now must be in charge of accounting the userspace memory quota.

## General Design

Use a tri-state reference count to track memory segments. I'm careful to say "segments" and not pages because it's possible that a solution where multiple pages are chunked into a single segment is best. This segment may even be dynamically "split" into sub-segments that can each be managed independently.

Most memory starts on the *untyped* state: this is memory neither used by the kernel nor userspace. The other 2 possible states are *kernel memory* and *user memory*. It shall only be possible to transition from either of those two states from the *untyped* state. Transitioning from *user* or *kernel* memory requires tracking a reference count. Only when the reference count is 0, can a transition be made to the *untyped* state.

Any implementations must be thread-safe, lock-free, and wait-free. This makes things trickier...

## Reference Counting

The simplest approach works with a per-frame solution. At startup, we create a *retype* table that holds an entry per physical frame in the system. The entry consists of an atomic *state* and an atomic *counter*. The state can either by *UNTYPED*, *RETYPING*, *KERNEL*, *USER*. The *RETYPING* state isn't strictly necessary but it helps with the code flow and should pose little overhead.

### Frame Types

An *UNTYPED* frame can be moved into a *RETYPING* state atomically. An *UNTYPED* frame has no references to it and a *RETYPING* frame only has 1 abstract reference (i.e. an owner of the frame without the frame being mapped or used in memory).

A frame in a *RETYPING* state can be converted to either *KERNEL* or *USER* without the need for validation as the ownership guarantees uniqueness.

A frame that is in this *RETYPING* state applies to the `UntypedFrame` struct.

Once a frame has been turned into *KERNEL* or *USER*, its reference count must be tracked.

The `KernelFrame` and `UserFrame` can perform some level of tracking, but these are always short-lived structures that must be turned into other entities in the system.

### Counting Kernel Frames

The presence of a `KernelFrame` tracks its reference count (incrementing it on `Clone`) and decrementing it on `Drop`. This guarantees that the frame won't change types underneath you. As I mentioned, these are short-lived. `KPtr` is a long-lived pointer that can be stored in the capability tables and can be constructed from an `UntypedFrame` or unsafely converted from a `KernelFrame` (which requires that the memory stored matches the expected type).

## Counting User Frames

As with kernel frames, the presence of a `UserFrame` tracks its reference count. However, these are again short-lived. User frames are directly mapped into user-level page tables. This means that the underlying `map` and `unmap` operations have to track the reference count. However, the reference counts must be decremented **in response** to the TLB flush of the unmap operation. Likewise, the reference count must be incremented right **before** a page is mapped to user-space. Failure to do so in this order may lead to the kernel and userspace pages to share memory segments!

## Managing Untyped Memory Resources

As with any other resource, untyped memory must be handled by the capability system. Ideally, components can have page-level granularity to the untyped memory resources -- meaning that some component may only have access to specific frames in untyped memory. 

Tracking untyped memory segments in capability tables is infeasible given the resource-space of memory (one capability entry per frame would blow up the capability table). Tracking the memory capability as memory regions would be a much better alternative to this.

Another option would be to manage physical memory resources in page tables. We already use page tables to manage memory capabilities (i.e. memory accesses go through page tables not capability tables). Similarly, we could use page tables to track untyped virtual memory (UVM). A portion of the virtual address space will be reserved by the kernel to track UVM mappings for each component. The mappings would not be user accessible but the precense of a mapping would indicate the capability to the frame. Alternatively, an extra page table entry bit could be used to indicate the validity of the frame

# Page Table Capabilities (x64)

Userspace processes manage their own set of page tables using the capability system. This capability poses a concern since the kernel must trust that some of the memory in the page tables belong to the kernel. In other words, there must be a section of virtual address space that the kernel can trust. That's where the kernel code, statics, stack, and other kernel memory live. Modern operating systems usually reserve the higher half of the memory space for kernel memory. For a 64 bit system with 48 bit addressing that would be 0xFFFF_8000_0000_0000 and above. Generally speaking, every page table should contain the same top half of entries. Notably, since there is no memory allocation in the kernel after boot, these entries will never change.

The limine bootloader already loads the kernel on the top 2GiB of memory and provides a higher-half phyiscal memory mapping. Any allocations done at boot for the kernel must be mapped in the top half or they must use the direct memory offset.

Allocations made for the init process (such as ELF loading and stack) must be done in the bottom half of the memory range.

Note that upon creating a new L4 page table all of the kernel entries will be copied. Luckily, since these don't change during runtime, only a shallow, top-level copy is needed.

Page tables must be properly typed to their level as it should not be allowed to have recursive mappings since that could compromise the integrity of the kernel entries. It's unclear that recursive mappings would be of any use in userspace. 

- L4 Tables (bottom half)
	* \[un\]map entry to L3 table
	* Change access rights
* L3 Tables
	* \[un\]map entry to L2 table
	* \[un\]map entry to 1GiB frame
	* Change access rights
* L2 Tables
	* \[un\]map entry to L1 table
	* \[un\]map entry to 2MiB frame
	* Change access rights
* L1 Tables
	- \[un\]map entry to frame
	* Change access rights


Unfortunately, page table operations must be atomic. Even if we avoided multiple references to page table capabilities over multiple components, a component with multiple threads may try to manipulate the same page table from different threads. This is not a huge problem because page table manipulation is likely infrequent and done within non-real-time components anyway.