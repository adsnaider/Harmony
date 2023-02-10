use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=kernel/src");
    println!("cargo:rerun-if-env-changed=BUILD_PROFILE");
    println!("cargo:rerun-if-env-changed=KERNEL_LOG_LEVEL");
    // set by cargo, build scripts should use this directory for output files
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let cargo = std::env::var("CARGO").unwrap();
    let profile = std::env::var("BUILD_PROFILE").unwrap_or("dev".to_owned());
    let dir = match profile.as_ref() {
        "dev" => "debug",
        "release" => "release",
        _ => panic!("Unknwon profile: {}", profile),
    };
    let target = format!("kernel/target/x86_64-unknown-none/{dir}/kernel");
    // set by cargo's artifact dependency feature, see
    // https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#artifact-dependencies
    let out = Command::new(cargo)
        .current_dir("kernel/")
        .arg("build")
        .arg(format!("--profile={profile}"))
        .status()
        .expect("Failed to build the kernel.");
    if !out.success() {
        panic!("Failed to compile kernel: {}", out,);
    }

    let kernel = Path::new(&target);

    // create an UEFI disk image (optional)
    let uefi_path = out_dir.join("uefi.img");
    bootloader::UefiBoot::new(&kernel)
        .create_disk_image(&uefi_path)
        .unwrap();

    // create a BIOS disk image
    let bios_path = out_dir.join("bios.img");
    bootloader::BiosBoot::new(&kernel)
        .create_disk_image(&bios_path)
        .unwrap();

    // pass the disk image paths as env variables to the `main.rs`
    println!("cargo:rustc-env=UEFI_PATH={}", uefi_path.display());
    println!("cargo:rustc-env=BIOS_PATH={}", bios_path.display());
}
