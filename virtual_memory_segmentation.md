# How does the kernel section memory?

As of this writing, all kernel and system memory lives in the top half of the
virtual memory space (i.e. 0xFFFF800000000000 for 64 bit kernel). There's one
notable exclusion at the moment and that is MMIO which lives in the identity
mapped address.

Within the top half, the kernel divides memory in the following way:

* 0xFFFF'8000'0000'0000 - 0xFFFF'9000'0000'0000 (16TB) : Kernel text, data, and 
bootloader statics.
* 0xFFFF'9000'0000'0000 - 0xFFFF'A000'0000'0000 (16TB) : Kernel heap.
* 0xFFFF'A000'0000'0000 - 0xFFFF'EFFF'FFBF'0000 (80TB) : MMIO
* 0xFFFF'EFFF'FFBF'0000 - 0xFFFF'EFFF'FFFF'0000 (4MiB) : Kernel stack.
* 0xFFFF'EFFF'FFFF'0000 - 0xFFFF'F000'0000'0000 (64KB) : Kernel statics.
* 0xFFFF'F000'0000'0000 - 0xFFFF'FFFF'FFFF'FFFF (16TB) : Physical memory. 
