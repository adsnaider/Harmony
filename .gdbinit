add-symbol-file .build/release/kernel
add-symbol-file .build/booter
layout split
set trace-commands on
set logging enabled on
target remote localhost:1234
