# AthenaOS

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

Building the final image and running it is all controlled in the `builder` crate.

From the `builder/` directory, do

`cargo run -- -k uefi emulate` or `cargo run --release -- -k uefi emulate`

This should launch qemu and you should be able to see the OS running.

### Building an ISO image

From the `builder/` directory, do

`cargo run -- -k uefi build athena.iso` or `cargo run --release -- -k uefi build athena.iso`

This will build the ISO image and save it to `athena.iso`. You can flash this
image to a USB drive for instance and boot from it to see the OS running on
actual hardware.


### Configuration

Configurations are passed to the builder through environment flags. Currently
these are the possible configurations:

* KERNEL_LOG_LEVEL [`debug`|`info`|`warn`|`error`] - controls the log level
(Defaults to `info`).

The only hardware architecture that is currently supported is x86_64.
