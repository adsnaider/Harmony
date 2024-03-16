add-symbol-file .build/release/kernel -o 0xffffffff80000000
add-symbol-file .build/booter
layout split
set trace-commands on
set logging enabled on
target remote localhost:1234
