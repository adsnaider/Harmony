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

ARTIFACTS = .build/

.PHONY: build bootimage emulate clean check ktest test

all: bootimage

# check:
	# cargo check --target $(TARGET) --tests --exclude=kernel

clippy:
	cargo clippy --target $(TARGET) --tests

build:
	@mkdir -p $(ARTIFACTS)/tests
	$(eval KERNEL_BIN=`cargo build --profile ${PROFILE} --target $(TARGET) --message-format=json | ./extract_exec.sh`)
	@ln -fs "$(KERNEL_BIN)" $(ARTIFACTS)/kernel
	# $(eval KERNEL_TEST_BIN=`cargo test --profile ${PROFILE} --target $(TARGET) --no-run --message-format=json | ./extract_exec.sh`)
	# @ln -fs "$(KERNEL_TEST_BIN)" $(ARTIFACTS)/tests/kernel

bootimage: build
	@mkdir -p $(ARTIFACTS)/tests
	cargo run -p builder --profile ${PROFILE} -- -k $(ARTIFACTS)/kernel -o ${ARTIFACTS}
	# cargo run -p builder --profile ${PROFILE} -- -k $(ARTIFACTS)/tests/kernel  -o ${ARTIFACTS}/tests

emulate: bootimage
	@./go.sh 33 qemu-system-x86_64 \
		-drive if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/OVMF.fd \
		-drive format=raw,file=$(ARTIFACTS)/uefi.img \
		-chardev stdio,id=char0,logfile=serial.log,signal=off \
		-serial chardev:char0 \
		$(QEMU_ARGS)

test:
	cargo test --workspace --exclude kernel

# ktest: bootimage
# 	@./go.sh 33 qemu-system-x86_64 \
# 		-drive if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/OVMF.fd \
# 		-drive format=raw,file=$(ARTIFACTS)/tests/uefi.img \
# 		-chardev stdio,id=char0,logfile=test.log,signal=off \
# 		-serial chardev:char0 \
# 		-device isa-debug-exit,iobase=0xf4,iosize=0x04 \
# 		-display none \
# 		$(QEMU_ARGS)


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
	rm -rf $(ARTIFACTS)/*
	cargo clean
