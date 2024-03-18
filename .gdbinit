add-symbol-file .build/debugger/kernel
add-symbol-file .build/debugger/booter
layout split
set trace-commands on
set logging enabled on
target remote localhost:1234
