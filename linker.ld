ENTRY (kmain)

SECTIONS
{
	# Kernel starts at second half of address range.
	. = 0xFFFF800000000000;
	.text ALIGN (4K): 
	{
		*(.text)
	}
	.rodata ALIGN (4K):
	{
		*(.rodata)
	}
	.data ALIGN (4K):
	{
		*(.data)
	}
	.bss ALIGN (4K):
	{
		*(COMMON)
		*(.bss)
		*(.bootstrap_stack)
	}
	/* Add a symbol that indicates the end address of the kernel. */
	_kernel_end = .;
}