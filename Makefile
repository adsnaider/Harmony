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

ARTIFACTS = .build

.PHONY: build bootimage emulate clean 

build:
	cargo build -p kernel --profile ${PROFILE} --target $(TARGET)
	@ln -fs $(realpath $(KERNEL_BIN)) $(ARTIFACTS)

bootimage: build
	@mkdir -p $(ARTIFACTS)
	cargo run -p builder --profile ${PROFILE} -- -k ${KERNEL_BIN} -o ${ARTIFACTS}

emulate: bootimage
	qemu-system-x86_64 \
		-drive if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/OVMF.fd \
		-drive format=raw,file=$(ARTIFACTS)/uefi.img \
		$(QEMU_ARGS)

iso: bootimage
	@mkdir -p $(ARTIFACTS)
	@rm -rf /tmp/iso
	@mkdir /tmp/iso
	@cp $(ARTIFACTS)/uefi.img /tmp/iso
	mkisofs -R \
			-f \
			-e uefi.img \
			-no-emul-boot \
			-V "Athena OS" \
			-o $(ARTIFACTS)/athena.iso \
			/tmp/iso
		
clean:
	rm -rf .build/*
	cargo clean
