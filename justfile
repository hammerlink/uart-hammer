build:
    # native x86_64 Linux
    cargo build --release --target x86_64-unknown-linux-gnu

    # RISC-V 64 (Linux)
    cargo zigbuild --release --target riscv64gc-unknown-linux-musl
