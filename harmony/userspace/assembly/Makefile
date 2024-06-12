all: hello.bin nop.bin

hello.bin: hello.asm
	nasm -felf64 hello.asm
	ld hello.o -o hello.bin

nop.bin: nop.asm
	nasm -felf64 nop.asm
	ld nop.o -o nop.bin

clean:
	rm -f *.o *.bin
