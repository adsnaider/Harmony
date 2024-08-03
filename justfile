set dotenv-load


arch := env_var("ARCH")
_target := arch + "-unknown-none"

check:
	cargo check --target {{_target}} --tests

clippy:
	cargo clippy --target {{_target}} --tests

