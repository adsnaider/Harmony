use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=programs/");
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rustc-link-arg=-Tathena/kernel/linker.ld");
    let status = Command::new("make")
        .args(["-C", "programs/"])
        .status()
        .expect("Failed to run makefile for init binaries");
    assert!(status.success(), "Make error");
}
