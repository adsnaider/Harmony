add-symbol-file .build/release/kernel -o 0xffffffff80000000
layout split
set trace-commands on
set logging enabled on
target remote localhost:1234
