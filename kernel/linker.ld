OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
}

SECTIONS
{
    . = 0x200000;
    _kernel_start = .;
    .text : ALIGN(0x1000) {
        _text_start = .;
        KEEP(*(.text.entry)) *(.text .text.*)
        . = ALIGN(0x1000);
        _text_end = .;
    } :text
    . = ALIGN(0x1000);
    .rodata : {
        _rodata_start = .;
        *(.rodata .rodata.*)
        . = ALIGN(0x1000);
        _rodata_end = .;
    } :rodata
    . = ALIGN(0x1000);
    .data : { _data_start = .; *(.data .data.*) } :data
    . = ALIGN(0x1000);
    .bss : { *(COMMON) *(.bss .bss.*) } :data
    . = ALIGN(0x1000);
    _data_end = .;
    _kernel_end = .;
    /DISCARD/ : { *(.eh_frame .eh_frame_hdr .comment .note .note.*) }
}
