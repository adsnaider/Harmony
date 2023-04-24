use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=programs/");
    Command::new("make")
        .args(["-C", "programs/"])
        .status()
        .expect("Failed to run makefile for init binaries");
}
