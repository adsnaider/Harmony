set dotenv-load

arch := env_var("ARCH")
profile := env_var("PROFILE")

_target := arch + "-unknown-none"

artifact_dir := ".build"
build_dir := artifact_dir / profile
image_path := build_dir / "harmony.iso"
_extractor := "jq -r '.filenames | last' | tail -2 | head -1"

check:
	cargo check --target {{_target}} --tests

clippy:
	cargo clippy --target {{_target}} --tests

setup:
	rm -rf {{build_dir}}
	mkdir -p {{build_dir}}
	mkdir -p {{artifacts_dir}}/debugger/

build-booter: setup
	#!/usr/bin/env bash
	set -euo pipefail
	export RUSTFLAGS="-Clink-arg=-no-pie -Crelocation-model=static"
	BOOTER_BIN=`cargo build -p booter --profile {{profile}} --target {{_target}} --message-format=json | {{_extractor}}`
	cp "$BOOTER_BIN" "{{build_dir}}/booter"
	ln -sf "{{profile}}/booter" "{{artifact_dir}}/booter"

build-kernel: setup build-booter
	#!/usr/bin/env bash
	set -euo pipefail
	KERNEL_BIN=`cargo build --profile {{profile}} --target {{_target}} --message-format=json | {{_extractor}}`
	cp -fs "$KERNEL_BIN" "{{build_dir}}/kernel"
	KERNEL_TEST_BIN=`cargo test --profile {{profile}} --target {{_target}} --no-run --message-format=json | {{_extractor}}`
	cp -fs "$KERNEL_TEST_BIN" "{{build_dir}}/kernel_test"

build: build-kernel

clean:
	rm -rf {{artifact_dir}}
	cargo clean
