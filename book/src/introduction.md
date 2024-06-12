# Introduction

TODO: GITHUB LINK

NAME is a microkernel operating system. It draws heavily from the design
of [Composite OS](https://github.com/gwsystems/composite), a research operating
system that pushes the boundaries of what user-level components can do.

As you will soon find out, NAME takes a very firm stance towards pushing
as many subsystems as are possible to userspace. This includes things like
drivers and servers, but also memory management and thread scheduling. This
design allows our kernel to be lock-free and wait-free, provides maximum
flexibility to userspace to define their policies, and reduces the attack
vector on the highly-privileged kernel. 
 
Throughout this book, we will explore the design and implementation of NAME

