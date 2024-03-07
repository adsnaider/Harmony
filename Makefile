TARGET ?= x86_64-unknown-none
PROFILE ?= dev
DEBUGGER ?= no
QEMU_ARGS ?=
ARTIFACTS = .build/
BUILD_DIR=$(ARTIFACTS)/$(PROFILE)
IMAGE_NAME=$(BUILD_DIR)/athena.iso
ISO_ROOT="$(BUILD_DIR)/iso_root"

ifeq "$(DEBUGGER)" "yes"
	QEMU_ARGS += -s -S
endif

# Convenience macro to reliably declare user overridable variables.
define DEFAULT_VAR =
    ifeq ($(origin $1),default)
        override $(1) := $(2)
    endif
    ifeq ($(origin $1),undefined)
        override $(1) := $(2)
    endif
endef

# Toolchain for building the 'limine' executable for the host.
override DEFAULT_HOST_CC := cc
$(eval $(call DEFAULT_VAR,HOST_CC,$(DEFAULT_HOST_CC)))
override DEFAULT_HOST_CFLAGS := -g -O2 -pipe
$(eval $(call DEFAULT_VAR,HOST_CFLAGS,$(DEFAULT_HOST_CFLAGS)))
override DEFAULT_HOST_CPPFLAGS :=
$(eval $(call DEFAULT_VAR,HOST_CPPFLAGS,$(DEFAULT_HOST_CPPFLAGS)))
override DEFAULT_HOST_LDFLAGS :=
$(eval $(call DEFAULT_VAR,HOST_LDFLAGS,$(DEFAULT_HOST_LDFLAGS)))
override DEFAULT_HOST_LIBS :=
$(eval $(call DEFAULT_VAR,HOST_LIBS,$(DEFAULT_HOST_LIBS)))

.PHONY: build emulate iso setup clean

all: build

setup:
	@rm -rf $(BUILD_DIR)
	@mkdir $(BUILD_DIR)

build: setup
	$(eval KERNEL_BIN=`cargo build --profile ${PROFILE} --target $(TARGET) --message-format=json | ./extract_exec.sh`)
	@cp "$(KERNEL_BIN)" "$(BUILD_DIR)/kernel"

emulate: iso
	@./go.sh 33 qemu-system-x86_64 \
		-cdrom $(IMAGE_NAME) \
		-bios /usr/share/ovmf/OVMF.fd \
		-chardev stdio,id=char0,logfile=serial.log,signal=off \
		-serial chardev:char0 \
		$(QEMU_ARGS)

limine:
	git clone https://github.com/limine-bootloader/limine.git --branch=v7.x-binary --depth=1
	$(MAKE) -C limine \
		CC="$(HOST_CC)" \
		CFLAGS="$(HOST_CFLAGS)" \
		CPPFLAGS="$(HOST_CPPFLAGS)" \
		LDFLAGS="$(HOST_LDFLAGS)" \
		LIBS="$(HOST_LIBS)"

iso: limine build
	rm -rf $(ISO_ROOT)
	mkdir -p $(ISO_ROOT)/boot
	cp -v $(BUILD_DIR)/kernel $(ISO_ROOT)/boot/
	mkdir -p $(ISO_ROOT)/boot/limine
	cp -v limine.cfg limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin $(ISO_ROOT)/boot/limine/
	mkdir -p $(ISO_ROOT)/EFI/BOOT
	cp -v limine/BOOTX64.EFI $(ISO_ROOT)/EFI/BOOT/
	cp -v limine/BOOTIA32.EFI $(ISO_ROOT)/EFI/BOOT/
	xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		$(ISO_ROOT) -o $(IMAGE_NAME)
	./limine/limine bios-install $(IMAGE_NAME)
	rm -rf $(ISO_ROOT)
		
clean:
	rm -rf $(ARTIFACTS)/*
	rm -rf limine/
	cargo clean
