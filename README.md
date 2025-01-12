# Harmony

Harmony OS (pronounced "harmonious") is an experimental, hobby OS written entirely in Rust.

## Getting Started

### Dependencies

I've only tested this on an x86_64 linux computer. You will need the following
tools to build and emulate the OS.

* [`just`](https://github.com/casey/just?tab=readme-ov-file#installation) for running commands

After installing it, you can run `just install-deps` to get the
remaining dependencies

### Emulation with QEMU

There are `just` rules for building and running the images.

`just emulate` or `just profile=release emulate`

This should launch qemu and you should be able to see the OS running. The
generated `serial.log` will include the full serial output.

Additionally you can set `debugger=yes` to run qemu with a remote debugger.

`just debugger=yes emulate`

Once QEMU is launched, you can open gdb on a terminal. The `.gdbinit` should
set everything up automatically for you so that you can insert breakpoints
and `c` to start debugging the program.

In order for this to work, you need to include the following in your 
`~/.gdbinit` 

```
set auto-load safe-path \
```

### Testing

Running `just test` will run unit tests across the entire project.

Running `just ktest` will run kernel integration tests on Qemu. This will
produce a `test.log` that contains the serial output.

### Building an ISO image

`just iso` or `just profile=release iso`

This will build the ISO image and save it to `.build/[profile]/harmony.iso`. You can flash this
image to a USB drive for instance and boot from it to see the OS running on
actual hardware.

### Configuration

Configuration options are passed through environment flags. Currently
these are the possible configurations:

* RUST_LOG [`trace` | `debug`| `info` | `warn` | `error`] - controls the log level
(Defaults to `info`).

The only hardware architecture that is currently supported is x86_64.
