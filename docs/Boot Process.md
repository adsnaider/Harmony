# System Initialization

On boot, the kernel will begin by initializing the system. This can be split into 2 steps: Initializing the hardware, and initializing the kernel structures

## Hardware Initialization

This section is architecture dependent, however, the end goal is mostly the same:

* Initialize the system to enable interrupts
* Enable system calls
* Setup virtual memory with direct mapping
* Setup the exception handlers
* Set up user level protection mode
* Initialize other cores

## Kernel Initialization

After initializing the hardware, we start initializing the kernel. This is mostly architecture independent

- Read through the memory map
- Set up a boot-only allocator
- Set up the retype table
- Set up the interrupt stack
- Initialize the boot component
	- Create its capability table
	- Create its page table(s)
		- Add physical regions
	- Define the boot capabilities
		- 0 -> Self Capability Table
		- 1 -> Self Page tables
	- Load ELF
- Long Jump to Boot component entry