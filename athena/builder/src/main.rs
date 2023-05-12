use std::path::PathBuf;

use clap::Parser;

/// Builds a bootimage for a kernel binary.
#[derive(Parser)]
#[command(author, version, about, long_about)]
pub struct Builder {
    /// The kernel image.
    #[arg(short, long)]
    kernel: PathBuf,
    /// Directory to place the UEFI and BIOS images.
    #[arg(short, long)]
    out_dir: PathBuf,
}

fn main() {
    let args = Builder::parse();
    // set by cargo, build scripts should use this directory for output files

    let uefi_path = args.out_dir.join("uefi.img");
    bootloader::UefiBoot::new(&args.kernel)
        .create_disk_image(&uefi_path)
        .unwrap();

    println!("UEFI image generated {uefi_path:?}");

    // create a BIOS disk image
    let bios_path = args.out_dir.join("bios.img");
    bootloader::BiosBoot::new(&args.kernel)
        .create_disk_image(&bios_path)
        .unwrap();

    println!("BIOS image generated {bios_path:?}");
}
