TARGET ?= x86_64-unknown-none

PROFILE ?= dev

DEBUGGER ?= no

QEMU_ARGS ?=

ifeq "$(DEBUGGER)" "yes"
	QEMU_ARGS += -s -S
endif

ifeq "$(PROFILE)" "dev"
	PROFILE_DIR := debug
else
	PROFILE_DIR := $(PROFILE)
endif

BUILD_DIR = target/$(TARGET)/$(PROFILE_DIR)/

KERNEL_BIN = $(BUILD_DIR)/kernel

ARTIFACTS = .build/

.PHONY: build bootimage emulate clean 

build:
	cargo build -p kernel --profile ${PROFILE} --target $(TARGET)
	@ln -fs $(realpath $(KERNEL_BIN)) $(ARTIFACTS)

bootimage: build
	@mkdir -p $(ARTIFACTS)/bootloader
	# cargo run -p builder --profile ${PROFILE} -- -k ${KERNEL_BIN} -o ${ARTIFACTS}


emulate: iso
	qemu-system-x86_64 \
		-drive if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/OVMF.fd \
		-drive format=raw,media=cdrom,file=$(ARTIFACTS)/athena.iso \
		$(QEMU_ARGS)

iso:
	@rm -rf /$(ARTIFACTS)/iso
	@mkdir -p $(ARTIFACTS)/iso
	@cp $(ARTIFACTS)/kernel $(ARTIFACTS)/iso/athena.elf
	@cp limine/limine.cfg limine/bin/limine.sys limine/bin/limine-cd.bin limine/bin/limine-cd-efi.bin $(ARTIFACTS)/iso
	@xorriso -as mkisofs \
			-b limine-cd.bin \
			-no-emul-boot \
			-boot-load-size 4 -boot-info-table \
			--efi-boot limine-cd-efi.bin \
			-efi-boot-part --efi-boot-image --protective-msdos-label \
			$(ARTIFACTS)/iso -o $(ARTIFACTS)/athena.iso
	@limine/bin/limine-deploy $(ARTIFACTS)/athena.iso
		
clean:
	rm -rf $(ARTIFACTS)/*
	cargo clean
