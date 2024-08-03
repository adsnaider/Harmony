set dotenv-load

arch := env_var("ARCH")
profile := env_var("PROFILE")
debugger := "no"

_target := arch + "-unknown-none"

artifact_dir := ".build"
build_dir := artifact_dir / profile
image_path := build_dir / "harmony.iso"
test_image_path := build_dir / "harmony-test.iso"
_extractor := "jq -r '.filenames | last' | tail -2 | head -1"
_iso_root := build_dir / "iso_root"
_qemu_user_args := env_var_or_default("QEMU_ARGS", "")
_qemu_args := if debugger == "yes" { _qemu_user_args + " -s -S" } else { _qemu_user_args }

check:
	cargo check --target {{_target}} --tests

clippy:
	cargo clippy --target {{_target}} --tests

setup:
	rm -rf {{build_dir}}
	mkdir -p {{build_dir}}

booter: setup
	#!/usr/bin/env bash
	set -euo pipefail
	export RUSTFLAGS="-Clink-arg=-no-pie -Crelocation-model=static"
	BOOTER_BIN=`cargo build -p booter --profile {{profile}} --target {{_target}} --message-format=json | {{_extractor}}`
	cp "$BOOTER_BIN" "{{build_dir}}/booter"
	ln -sf "{{profile}}/booter" "{{artifact_dir}}/booter"

kernel: setup booter
	#!/usr/bin/env bash
	set -euo pipefail
	KERNEL_BIN=`cargo build --profile {{profile}} --target {{_target}} --message-format=json | {{_extractor}}`
	cp -fs "$KERNEL_BIN" "{{build_dir}}/kernel"
	KERNEL_TEST_BIN=`cargo test --profile {{profile}} --target {{_target}} --no-run --message-format=json | {{_extractor}}`
	cp -fs "$KERNEL_TEST_BIN" "{{build_dir}}/kernel_test"

build: kernel

limine:
	{{path_exists("limine/")}} || git clone https://github.com/limine-bootloader/limine.git --branch=v7.x-binary --depth=1
	make -C limine

iso: limine build
	rm -rf {{_iso_root}}
	mkdir -p {{_iso_root}}/boot
	cp -v {{build_dir}}/kernel {{_iso_root}}/boot
	mkdir -p {{_iso_root}}/boot/limine
	cp -v limine.cfg limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin {{_iso_root}}/boot/limine
	mkdir -p {{_iso_root}}/EFI/BOOT
	cp -v limine/BOOTX64.EFI {{_iso_root}}/EFI/BOOT/
	cp -v limine/BOOTIA32.EFI {{_iso_root}}/EFI/BOOT/
	xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		{{_iso_root}} -o {{image_path}}
	@echo "ISO Image Built: {{image_path}}"


test-iso: limine build
	rm -rf {{_iso_root}}
	mkdir -p {{_iso_root}}/boot
	cp -v {{build_dir}}/kernel_test {{_iso_root}}/boot
	mkdir -p {{_iso_root}}/boot/limine
	cp -v limine-test.cfg limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin {{_iso_root}}/boot/limine
	mkdir -p {{_iso_root}}/EFI/BOOT
	cp -v limine/BOOTX64.EFI {{_iso_root}}/EFI/BOOT/
	cp -v limine/BOOTIA32.EFI {{_iso_root}}/EFI/BOOT/
	xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		{{_iso_root}} -o {{test_image_path}}
	@echo "Test ISO Image Built: {{test_image_path}}"

dbg_dir: setup
	mkdir -p {{artifact_dir}}/debugger/
	ln -sf {{build_dir}}/kernel {{artifact_dir}}/debugger
	ln -sf {{build_dir}}/kernel_test {{artifact_dir}}/debugger
	ln -sf {{build_dir}}/booter {{artifact_dir}}/debugger

emulate: dbg_dir iso
	@./go.sh 33 qemu-system-x86_64 \
		-cdrom {{image_path}} \
		-bios /usr/share/ovmf/OVMF.fd \
		-chardev stdio,id=char0,logfile=serial.log,signal=off \
		-serial chardev:char0 \
		{{_qemu_args}}

ktest: test-iso
	@./go.sh 33 qemu-system-x86_64 \
		-cdrom {{test_image_path}} \
		-bios /usr/share/ovmf/OVMF.fd \
		-chardev stdio,id=char0,logfile=serial.log,signal=off \
		-serial chardev:char0 \
		-device isa-debug-exit,iobase=0xf4,iosize=0x04 \
		-display none \
		{{_qemu_args}}

clean:
	rm -rf {{artifact_dir}}
	rm -rf limine/
	cargo clean
