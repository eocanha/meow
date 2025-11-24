ROOT_DIR:=$(dir $(realpath $(firstword $(MAKEFILE_LIST))))

all:	build install

build:
	cargo build

clean:
	cargo clean

install:
	cargo install --path $(ROOT_DIR)

uninstall:
	cargo uninstall --path $(ROOT_DIR)
