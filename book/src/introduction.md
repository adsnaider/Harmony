# Introduction

This book describes Harmony operating system. All of the source code (including
this book) lives in <https://github.com/adsnaider/Harmony>

Harmony OS (pronounced "harmonious") is a microkernel operating system. It
draws heavily from the design of 
[Composite OS](https://github.com/gwsystems/composite), a research operating
system that pushes the boundaries of what user-level components can do. While
we adopt some of the design elements, the implementation is likely to differ
quite drastically (especially given that Harmony is written in Rust while
Composite is written in C).

As you will soon find out, Harmony takes a very firm stance towards pushing
as many subsystems as are possible to userspace. This includes things like
drivers and servers, but also memory management and thread scheduling. This
design allows our kernel to be non-preemptive, in part by heavily relying on
atomic operations to provide a wait-free interface.

The usual benefits of a microkernel design revolve around reliability and
security. By adopting this design, we also adopt these traits. On top of that,
by making the kernel so small and (externally) simple, we also get the chance to
make our kernel wait-free, enabling a non-preemptive kernel. We will talk more
about these benefits later on.
 
Throughout this book, we will explore the design and implementation of Harmony.
