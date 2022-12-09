# Define compilation arch.
ARCH ?= x86_64
# Cargo compilation profile. 
PROFILE ?= dev
# Base path of the workspace.
BASE ?= $(shell pwd)
# Extra flags for QEMU.
QEMUFLAGS ?= 

dev_CARGO_DIR = debug
release_CARGO_DIR = release
# Cargo's compilation directory for appropriate profile.
CARGO_DIR = $($(PROFILE)_CARGO_DIR)

# EFI Executable name.
x86_64_EFI = BOOTX64.EFI
ARCHEFI = $($(ARCH)_EFI)

# Build directory for this compilation.
BUILD_DIR = $(BASE)/.build/$(ARCH)

# Crates.
BOOTLOADER = bootloader
KMAIN = kmain

# Cargo compilation targets.
CARGO_EFI = $(BASE)/target/x86_64-unknown-uefi/$(CARGO_DIR)/$(BOOTLOADER).efi
CARGO_KERNEL = $(BASE)/target/x86_64-unknown-none/$(CARGO_DIR)/libkmain.a

# Artifacts directory needed for image.
ARTIFACTS = $(BUILD_DIR)/artifacts
# Artifacts generated.
EFI = $(ARTIFACTS)/$(ARCHEFI)
KERNEL = $(ARTIFACTS)/kernel
FONT = $(BASE)/fonts/FONTS/SYSTEM/FREEDOS/CPIDOS30/CP113.F16

# Actual image directory. Eventually, this will become an ISO file.
IMAGE_ROOT = $(BUILD_DIR)/image

TARGET = $(ARCH)-unknown-none

.PHONY: $(CARGO_EFI) $(CARGO_KERNEL) boot_image clean all env check

all: boot_image

env:
	mkdir -p $(ARTIFACTS)
	mkdir -p $(IMAGE_ROOT)/EFI/BOOT

check:
	cd $(BOOTLOADER) && cargo clippy
	cd $(KMAIN) && cargo clippy

$(CARGO_EFI):
	# We cd to run the command because we want to use the .cargo/config.toml
	cd $(BOOTLOADER) && cargo build --profile $(PROFILE) --target $(ARCH)-unknown-uefi

$(EFI): $(CARGO_EFI) env
	cp $(CARGO_EFI) $(EFI)


$(CARGO_KERNEL):
	# We cd to run the command because we want to use the .cargo/config.toml
	cd $(KMAIN) && cargo build --profile $(PROFILE) --target $(TARGET)

$(KERNEL): $(CARGO_KERNEL) linker.ld env
	ld -T linker.ld -o $(KERNEL) $(CARGO_KERNEL)

boot_image: $(EFI) $(KERNEL) env
	echo "\EFI\BOOT\$(ARCHEFI)" > $(IMAGE_ROOT)/startup.nsh
	cp $(EFI) $(IMAGE_ROOT)/EFI/BOOT/$(ARCHEFI)
	cp $(KERNEL) $(IMAGE_ROOT)/kernel
	cp $(FONT) $(IMAGE_ROOT)/font.bdf

run: boot_image
	qemu-system-$(ARCH) \
		-drive if=pflash,format=raw,readonly,file=/usr/share/ovmf/OVMF.fd \
		-drive format=raw,file=fat:rw:$(IMAGE_ROOT) \
		$(QEMUFLAGS)

run_with_gdb: boot_image
	qemu-system-$(ARCH) \
		-drive if=pflash,format=raw,readonly,file=/usr/share/ovmf/OVMF.fd \
		-drive format=raw,file=fat:rw:$(IMAGE_ROOT) \
		-s -S $(QEMUFLAGS)

gdb: $(KERNEL)
	gdb $(KERNEL)

clean:
	rm -rf build/
	cargo clean
