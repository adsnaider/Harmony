target remote localhost:1234
add-symbol-file .build/kernel -o 0xFFFF800000000000
layout split
set trace-commands on
set logging enabled on