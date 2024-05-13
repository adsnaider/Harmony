# Purpose

Define a thin API to that enables granular access to physical and abstract system resources from user space.

# Goals
* High granularity, capability based
* Thread safe
* Thin API that enables user space to define higher-level policies
* Lock-free
	* Wait-free ideally
* No memory management

# General Design

We define capability tables/capability trees. These are integer tries. Each node in the trie occupies one block size, allowing userspace to allocate a new node through the use of Untyped Frames. Each capability occupies a slot in the trie's node. The capability always refers to a [resource](#Resources) and optionally some resource-specific protection flags akin to r/w/x. 

A userspace process will trigger a syscall with the capability ID as well as some resource-specific operation. The kernel performs all the necessary validations to guarantee the operation is valid and allowed by the capability before performing the operation.
# Resources

Resources in the system encompass two general kinds:

## Physical System Resources

The physical resources are basically the resources provided by the hardware. These are often protected from user-space at a hardware level and the kernel must allow specific components to access these privileged protection domains.

* Access to ports
* Physical Memory
* Memory mapped I/O
* Interrupts/notifications
## Abstract Resources

These are resources provided by the kernel to aid in bootstrapping the rest of the policy in the kernel. These often refer to the underlying kernel data structures and provide higher abstractions used to orchestrate the system.

- Threads
- Page Tables
- Capability Tables
- Synchronous IPC
- Asynchronous IPC

# Thread Safety

Capabilities must be thread safe. This is an inherent need from construction as capabilities are associated with components and each component may be "serving" many threads at once. How this is managed is on a resource-by-resource basis as each one will likely require a different approach. Most resources will be depicted by a pointer to a kernel frame. In this case a reference count will be used as explained in [[Memory Management]].

## Resource Operations

### Threads

| Operation    | Description                                                                                                             | Notes                                                                                              | Thread Safety                             |
| ------------ | ----------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| Activate     | Activates the thread, effectively switching core exeuction to that thread and saving the contents of the current thread | A thread can only be activated if its both inactive and its affinity is the current cpu's affinity | Core-local makes it trivially thread safe |
| Set Affinity | Moves the thread to another core                                                                                        | A thread can only be moved with a syscall from the same core as the current thread's affinity      | Core-local makes it trivially thread safe |
| Introspect   | Provides information about this thread                                                                                  |                                                                                                    |                                           |

### Page Tables

| Operation    | Description                                                         | Notes                                                                                                                                                                                                                                                            | Thread Safety                                                                                                                                                    |
| ------------ | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Link         | Links a specific page table slot to a lower-rank page table o - Page tables are typed and can only be linked to another sequential page table or page<br>- Only lower-half entries are valid (rest are reserved for kernel)<br>- Flags are passed in here as well<br>- Requires capability to pointee frame or page table  r  r  r  r  | All operations are atomic but no guarantees can be made that the final state will match the requested state (e.g. if another thread is also modifying the table) |
| Unlink       | Unlinks a page/page table from the entry                            | - Only lower-half entri                                                                                                                                                                                                                                          | All operations are atomic (relaxed)                                                                                                                              |
| Change flags | Changes the flags of the page table entry                           | - Only lower-half e                                                                                                                                                                                                                                              | All operations are atomic (relaxed)                                                                                                                              |

### Capability Tables

| Operation | Description                                       | Notes                                                                    | Thread Safety                  |
| --------- | ------------------------------------------------- | ------------------------------------------------------------------------ | ------------------------------ |
| Create    | Creates a resource                                | Capability slot must be passed                                           | Atomic trie implementation     |
| Drop      | Drops a resource                                  | Destruction will only happen if no more references exist to the resource | Atomic reference count         |
| Copy      | Copies a capability from another capability table |                                                                          | Atomic reference count cloning |
| Link      | Links an entry to another Capability Table        |                                                                          | Atomic trie implementation     |
| Unlink    | Unlinks the entry                                 |                                                                          | Atomic trie implementation     |

### Synchronous Invocations

| Operation | Description                         | Notes | Thread Safety |
| --------- | ----------------------------------- | ----- | ------------- |
| Call      | Performs the synchronous invocation |       |               |

### Asynchronous Invocations

### Memory Regions

| Operation | Description                                       | Notes                                                   | Thread Safety                  |
| --------- | ------------------------------------------------- | ------------------------------------------------------- | ------------------------------ |
| Retype    | Attempts to retype a section of the memory region | As of now, only individual pages can be retyped at once | Atomic operations for retyping |
| Split     | Splits a region into 2 capabilities               |                                                         | Immutable                      |


