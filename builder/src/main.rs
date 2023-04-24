use std::path::PathBuf;
use std::process::Command;

use clap::{Parser, Subcommand, ValueEnum};

/// Simple script for emulating or building the kernel.
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about)]
struct Args {
    /// Which kind of built image to use.
    #[arg(value_enum, short, long)]
    kind: ImageKind,
    /// Action to execute.
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, ValueEnum)]
enum ImageKind {
    /// The UEFI image.
    Uefi,
    /// The legacy BIOS image.
    Bios,
}

/// Possible builder commands.
#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Use qemu to emulate the kernel.
    Emulate {
        #[arg(short, long, default_value_t = false)]
        debug: bool,
    },
    /// Build an ISO image
    Build {
        /// Where to place the built ISO.
        output: PathBuf,
    },
}

fn main() {
    let args = Args::parse();
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    match args.command {
        Commands::Emulate { debug } => {
            let mut cmd = Command::new("qemu-system-x86_64");
            match args.kind {
                ImageKind::Uefi => {
                    cmd.arg("-drive")
                        .arg("if=pflash,format=raw,readonly,file=/usr/share/ovmf/OVMF.fd");
                    cmd.arg("-drive")
                        .arg(format!("format=raw,file={uefi_path}"));
                }
                ImageKind::Bios => {
                    cmd.arg("-drive")
                        .arg(format!("format=raw,file={bios_path}"));
                }
            }
            if debug {
                cmd.arg("-S").arg("-s");
            }
            let mut child = cmd.spawn().unwrap();
            child.wait().unwrap();
        }
        Commands::Build { output } => {
            let output = output.into_os_string().into_string().unwrap();
            Command::new("rm")
                .args(["-rf", "/tmp/iso"])
                .output()
                .unwrap();
            Command::new("mkdir")
                .args(["-p", "/tmp/iso"])
                .output()
                .unwrap();

            match args.kind {
                ImageKind::Uefi => {
                    Command::new("cp")
                        .args([uefi_path, "/tmp/iso"])
                        .output()
                        .unwrap();
                    Command::new("mkisofs")
                        .args([
                            "-R",
                            "-f",
                            "-e",
                            "uefi.img",
                            "-no-emul-boot",
                            "-V",
                            "Athena OS",
                            "-o",
                            &output,
                            "/tmp/iso",
                        ])
                        .output()
                        .unwrap();
                }
                _ => panic!("Only UEFI images can be made into ISOs at this time."),
            }
        }
    }
}
