# UEFI Bootloader

The bootloader is the initial point of entry of AtehnaOS. The job of the
bootloader is to load the kernel into memory, retrieve access to different
systems in the computer (such as the framebuffer, memory map, etc), and finally
execute the kernel's entry point.
