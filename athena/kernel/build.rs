use std::process::Command;

fn main() {
    Command::new("make")
        .args(["-C", "programs/", "all"])
        .status()
        .expect("Failed to run makefile for init binaries");
}
