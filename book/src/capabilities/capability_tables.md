# Capability Tables

Capabilities to capability tables. Very meta, but very important. The idea is
simple in hindsight. We could put a lot of effort in setting up a capability
system with some implementation for capability delegation, revocation, and id
management or we could instead throw it up to userspace to do this :)

Hopefully the message is clear by this point, we don't want unnecessary
complexity in the kernel. Especially something like capability management can
become quite complex in a non-preemptive kernel since delegation and revocation
can be O(N) operations.

## Terminology

Before moving further, I want to clarify some terminology.

* Capability Table: This is a single, frame-wide trie entry that stores a set of
capabilities. This could be either a root capability table (i.e what a component
uses when it searches for syscalls) or any linked table below.
* Resource Table: This is the root capability table used by a component. It's a sepcific
capability table that is used when a userspace component performs a syscall.

## The Foundation for Abstraction

By simply providing capabilities to capability tables userspace can bootstrap
its own environment. When the boot component is instantiated, it is given 3
capabilities:

0. Capability to its own capability table
1. Capability to its own thread
2. Capability to its own page table
3. Capability to all physical frames.

The first one is the most important as having a capability to its own table
enables resource creation and management. If you want to create a thread or
a page table, you need to have a capability to either your own capability table
or some other component's table.

Alongside resource creation, a capability to a capability table enables resource
deletion, copying (to another capability table), linking (for extending
the capability space) and unlinking.

This small sets of operations is enough to bootstrap a fully functional system
without requiring any capability management in the kernel.
