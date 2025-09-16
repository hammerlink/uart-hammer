build: build_x86_64 build_riscv_musl

build_x86_64:
    # native x86_64 Linux
    cargo build --release --target x86_64-unknown-linux-gnu

build_riscv_musl:
    # RISC-V 64 (Linux)
    cargo zigbuild --release --target riscv64gc-unknown-linux-musl

install_riscv_musl_toolchain:
    # Requirements, install zig
    cargo install cross
    cargo install cargo-zigbuild
    rustup target add riscv64gc-unknown-linux-musl
