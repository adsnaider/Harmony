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

From the top-level directory, run

`cargo run -- -k uefi emulate`

This should launch qemu and you should be able to see the OS running.

### Building an ISO image

From the top-level directory, run

`cargo run -- -k uefi build athena.iso`

This will build the ISO image and save it to `athena.iso`. You can flash this
image to a USB drive for instance and boot from it to see the OS running on
actual hardware.
