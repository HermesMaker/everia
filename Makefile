default: install

test:
	@cargo test -- --nocapture

install:
	@cargo install --path .

uninstall:
	@cargo uninstall everia
