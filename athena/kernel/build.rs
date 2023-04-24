use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=programs/");
    let status = Command::new("make")
        .args(["-C", "programs/"])
        .status()
        .expect("Failed to run makefile for init binaries");
    assert!(status.success(), "Make error");
}
