#g AthenaOS

Athena is an experimental, hobby OS written entirely in Rust.

## Getting Started

### Dependencies

I've only tested this on an x86_64 linux computer. You will need the following
tools to build and emulate the OS.

* [Rust](https://rustup.rs/)
* `qemu-system-x86_64` for emulating.
* OVMF (currently hardcoded to `/usr/share/ovmf/OVMF.fd`)
* `mkisofs` for building an ISO.

### Emulation with QEMU

The builder/ crate is a simple binary that can generate the UEFI or BIOS images.

There are make rules to make building/running the images easier.

`make emulate` or `PROFILE=release make emulate`

This should launch qemu and you should be able to see the OS running.

Additionally you can set `DEBUGGER=yes` to run qemu with a remote debugger.

`DEBUGGER=yes make emulate`

Once QEMU is launched, you can open gdb on a terminal

Run `target remote localhost:1234` to connect to the instance. To include
symbols, you should also add the the kernel binary as a relocatable library.

`add-symbol-file .build/kernel -o 0xFFFF800000000000`

From here you can just `c` to continue execution of the program after setting
up the breakpoints.

### Building an ISO image

`make iso` or `PROFILE=release make iso`

This will build the ISO image and save it to `.build/athena.iso`. You can flash this
image to a USB drive for instance and boot from it to see the OS running on
actual hardware.

### Configuration

Configuration is passed through environment flags. Currently
these are the possible configurations:

* KERNEL_LOG_LEVEL [`debug`|`info`|`warn`|`error`] - controls the log level
(Defaults to `info`).

The only hardware architecture that is currently supported is x86_64.
