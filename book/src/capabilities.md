# Capabilities

In Harmony, capabilities are the main resources used to control the system from
userspace. In fact, we only provide a single logical system call: Apply some
operation to a capability.

## So, What is a Capability?

A capability is a logical token used to manage resources in userspace. They are
managed by the kernel and are at the backbone of system isolation and control.
For a userspace component to be able to perform an action on the system, they
**must** have the capability to the appropriate resource.

Specifically, a capability is an entry in a capability table. This capability is
attached to some resource and the kernel provides a set of operations that can
be applied to different resources. Resources include:
 
- Threads
- Page Tables
- Synchronous Invocations
- Hardware access
- Capability Tables
- etc.

As you can see, capability tables themselves are resources abstracted by the
kernel. This is an intentional decision that enables our kernel to remain small.
As you may imagine, having a capaiblity to a capability table should not be done
lightly. As such, most userspace components will not have a capability to a
capability table.

## Capabilities are Just File Descriptors

Well, actually it makes more sense to think about it the other way around.
File descriptors are just capabilities. In a unix system, when you have a file
descriptor (represented as a number), you have a key to a resource (a file).
This key enables many operations to be performed on this file (read, write,
append, delete, etc.).

This idea can be extended to many resources in the system. For instance, by
having a capaiblity to a thread control block, one can "activate" it or delete
it. By having the capaiblity to a synchronous invocation, one can perform
synchronous IPC.

## Implementation

Capabilities are implemented in the kernel as a page-wide number trie. The
reason for this is that this data-structure plays well with Harmony's memory
management model which requires that dynamic kernel data-structures are a
page wide. A trie also has a constant-time time complexity. For instance, in a
system with 4k pages, assuming the trie-nodes are each 64 bytes (including the
pointers to the next level), each block would be able to store `4KB / 64B = 64`
nodes. That's 6 bits per level, so for a 32 bit key, we would gurantee a maximum
depth of 6. In general, the key lookup will always follow a `log(b)` where `b`
is the bit-width of the key.

