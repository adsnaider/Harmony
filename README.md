# Harmony

Harmony OS (pronounced "harmonious") is an experimental, hobby OS written entirely in Rust.

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

This should launch qemu and you should be able to see the OS running. The
generated `serial.log` will include the full serial output.

Additionally you can set `DEBUGGER=yes` to run qemu with a remote debugger.

`DEBUGGER=yes make emulate`

Once QEMU is launched, you can open gdb on a terminal. The `.gdbinit` should
set everything up automatically for you so that you can insert breakpoints
and `c` to start debugging the program.

In order for this to work, you need to include the following in your 
`~/.gdbinit` 

```
set auto-load safe-path \
```

### Testing

Running `make test` will run unit tests across the entire project.

Running `make ktest` will run kernel integration tests on Qemu. This will
produce a `test.log` that contains the serial output.

### Building an ISO image

`make iso` or `PROFILE=release make iso`

This will build the ISO image and save it to `.build/harmony.iso`. You can flash this
image to a USB drive for instance and boot from it to see the OS running on
actual hardware.

### Configuration

Configuration is passed through environment flags. Currently
these are the possible configurations:

* KERNEL_LOG_LEVEL [`debug`|`info`|`warn`|`error`] - controls the log level
(Defaults to `info`).

The only hardware architecture that is currently supported is x86_64.
