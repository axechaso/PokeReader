.PHONY: all clean lint test
LIBPOKEREADER := reader_core/target/armv6k-nintendo-3ds/release/libpokereader.a

R_SRCS := $(shell find reader_core/src time_lock_core/src -name '*.rs')
C_SRCS := $(shell find 3gx/sources -name '*.c')
H_SRCS := $(shell find 3gx/includes -name '*.h')

all: out/default.3gx out/PokeReader-Time.3dsx

$(LIBPOKEREADER): $(R_SRCS)
	cargo +nightly-2024-03-21 build --locked --release -Z build-std=core,alloc --target armv6k-nintendo-3ds --manifest-path reader_core/Cargo.toml

out/default.3gx: $(LIBPOKEREADER) $(C_SRCS) $(H_SRCS)
	make clean -C 3gx
	make -C 3gx
	mkdir -p out
	cp 3gx/build/3gx.3gx out/default.3gx

out/PokeReader-Time.3dsx: $(LIBPOKEREADER) configurator/source/main.c configurator/Makefile 3gx/sources/time_lock_io.c 3gx/includes/time_lock.h
	$(MAKE) -C configurator
	mkdir -p out
	cp configurator/build/PokeReader-Time.3dsx $@

clean:
	cargo clean --manifest-path reader_core/Cargo.toml
	make clean -C 3gx
	make clean -C configurator
	rm -rf out

format:
	cargo +nightly fmt --all --manifest-path reader_core/Cargo.toml

lint:
	cargo +nightly-2024-03-21 clippy --all-targets --manifest-path time_lock_core/Cargo.toml -- -D warnings
	cargo +nightly-2024-03-21 clippy --release -Z build-std=core,alloc --target armv6k-nintendo-3ds --manifest-path reader_core/Cargo.toml

test:
	cargo +nightly-2024-03-21 test --locked --manifest-path time_lock_core/Cargo.toml
	cargo +nightly-2024-03-21 test --locked --features test_stubs --manifest-path reader_core/Cargo.toml
