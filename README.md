# CPUID scanning and checking

## What is it?

A utility to identify a cpu and features present. It uses the `CPUID` instruction and on platforms that support it,
feature MSRs

## What is required to build it?

Rust build tools. How you get them depends on platform.

### Centos

The build tools are provided in the distro simply run the following

`sudo dnf install cargo rustfmt clippy`

### Debian/Ubuntu/Pop-OS

The build tools are provided in the distro simply run the following

`sudo apt install cargo`

### Generic Linux/MacOS/Windows

See instructions [here](https://www.rust-lang.org/tools/install) They will walk you through installing cargo and any
prerequisites.

## How is it built?

Simple

`cargo build`

## How can I run it?

Cargo will also run it. If you didn't build it, it will also do that.

`cargo run -- --help`

Cargo will place the executable depending on if it was built as a release or debug build.

`./target/debug/cpuinfo --help`
`./target/release/cpuinfo --help`

## What about CI?

This is currently a todo item. Until we set it up, `cargo clippy` is used to lint the code.

## How is code formatted?

Simple rustfmt

`cargo fmt`

## What needs work?

Probably many things ... 2 that come to mind worth mentioning are:
1. CPU Features - The current list was added to make the framework ... work and we need more.
2. Documentation - `cargo doc` generating a reasonable set from what is there would be nice.

Ideally, this section will grow some github issues or something.

### Where can cpu information be found?

The locations of the manuals do change so, no links here as they maybe broken.
1. Intel - look for "Intel® 64 and IA-32 Architectures Software Developer’s Manual"
  * "Volume 2" contains the CPUID instruction return information
  * "Volume 3" contains the VMX Features
  * "Volume 4" contains information on MSRs
2. AMD - "AMD64 Architecture Programmer’s Manual"
  * "Volume 3" Appendix D Contains CPUID information
