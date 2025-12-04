ROOT_DIR:=$(dir $(realpath $(firstword $(MAKEFILE_LIST))))

all:	build install

build:
	cargo build

# Requires samply. Install with: cargo install samply
profiling:
	cargo build --profile performance-profiling
	samply record ./target/performance-profiling/meow error

clean:
	cargo clean

install:
	cargo install --path $(ROOT_DIR)

uninstall:
	cargo uninstall --path $(ROOT_DIR)
