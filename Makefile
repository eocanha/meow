ROOT_DIR:=$(dir $(realpath $(firstword $(MAKEFILE_LIST))))

all:	build install

build:
	cargo build

# Requires samply. Install with: cargo install samply
profiling:
	CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release
	samply record ./target/release/meow error

clean:
	cargo clean

install:
	cargo install --path $(ROOT_DIR)

uninstall:
	cargo uninstall --path $(ROOT_DIR)
