.PHONY: lint
lint:
	@cargo fmt && cargo clippy
