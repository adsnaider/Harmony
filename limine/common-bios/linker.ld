OUTPUT_FORMAT(elf32-i386)
OUTPUT_ARCH(i386)
ENTRY(_start)
PHDRS
{
    text_s2 PT_LOAD FLAGS((1 << 0) | (1 << 2)) ;
    data_s2 PT_LOAD FLAGS((1 << 1) | (1 << 2)) ;
    text_s3 PT_LOAD FLAGS((1 << 0) | (1 << 2)) ;
    data_s3 PT_LOAD FLAGS((1 << 1) | (1 << 2)) ;
}
SECTIONS
{
    . = 0xf000;
    .text.stage2 : {
        *(.entry)
        *(.realmode)
        *.s2.o(.text .text.*)
        *libgcc*.a:*(.text .text.*)
    } :text_s2
    .data.stage2 : {
        build_id_s2 = .;
        KEEP(*build-id.s2.o(*))
        *.s2.o(.no_unwind)
        s2_data_begin = .;
        *.s2.o(.data .data.*)
        *libgcc*.a:*(.data .data.*)
        s2_data_end = .;
        *.s2.o(.rodata .rodata.*)
        *libgcc*.a:*(.rodata .rodata.*)
        *(.stage2_map)
    } :data_s2
    .text.stage3 : {
        stage3_addr = .;
        *(.text .text.*)
    } :text_s3
    .data.stage3 : {
        build_id_s3 = .;
        KEEP(*build-id.s3.o(*))
        *(.rodata .rodata.*)
        *(.full_map)
        *(.no_unwind)
        data_begin = .;
        *(.data .data.*)
    } :data_s3
    .note.gnu.build-id : {
        *(.note.gnu.build-id)
        limine_sys_size = . - 0xf000;
    } :data_s3
    .bss : {
        bss_begin = .;
        *(COMMON)
        *(.bss .bss.*)
        bss_end = .;
        data_end = .;
    } :data_s3
    /DISCARD/ : {
        *(.eh_frame)
        *(.note .note.*)
    }
}
